import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { extname, resolve, sep } from "node:path";

import { chromium, type Browser } from "playwright";
import { expect, test } from "vite-plus/test";

/**
 * Worker that bundles and evaluates one virtual entry per message. It imports the bundler after
 * it installs the handler, because the bundler awaits its WASM at the top level and a static
 * import would delay the handler until messages that arrive earlier are lost.
 */
const bundleWorker = `
self.onmessage = async ({ data: { files, entry } }) => {
  try {
    const { bundleModule } = await import("./bundle.js");
    const { code } = await bundleModule(new Map(files), entry);
    const url = URL.createObjectURL(new Blob([code], { type: "text/javascript" }));
    try {
      self.postMessage({ value: await (await import(url)).default() });
    } finally {
      URL.revokeObjectURL(url);
    }
  } catch (error) {
    self.postMessage({ error: String(error) });
  }
};
`;

test("the packaged browser bundler evaluates and reloads virtual configurations with its WASM workers", async () => {
  const directory = resolve(import.meta.dirname, "../dist/extension/dist/browser");
  const server = createServer(async (request, response) => {
    const pathname = new URL(request.url!, "http://localhost").pathname;
    response.setHeader("Cross-Origin-Opener-Policy", "same-origin");
    response.setHeader("Cross-Origin-Embedder-Policy", "require-corp");
    if (pathname === "/") {
      response
        .writeHead(200, {
          "Content-Type": "text/html",
        })
        .end("<!doctype html><title>Browser configuration</title>");
      return;
    }
    // The language server bundles in a worker. The bundler's WASM threads may block with
    // `Atomics.wait`, which the page's main thread rejects.
    if (pathname === "/bundle-worker.js") {
      response
        .writeHead(200, {
          "Content-Type": "text/javascript",
        })
        .end(bundleWorker);
      return;
    }
    const file = resolve(directory, "." + decodeURIComponent(pathname));
    if (!file.startsWith(directory + sep)) {
      response.writeHead(403).end();
      return;
    }
    try {
      const contents = await readFile(file);
      response
        .writeHead(200, {
          "Content-Type": extname(file) === ".wasm" ? "application/wasm" : "text/javascript",
        })
        .end(contents);
    } catch {
      response.writeHead(404).end();
    }
  });
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));

  let browser: Browser | undefined;
  try {
    const address = server.address();
    if (!address || typeof address === "string") {
      expect.fail("Missing browser fixture port");
    }
    browser = await chromium.launch({
      channel: "chrome",
      headless: true,
    });
    const page = await browser.newPage();
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    await page.goto(`http://127.0.0.1:${address.port}/`);
    const result = await page.evaluate(async () => {
      const worker = new Worker("/bundle-worker.js", { type: "module" });
      const entry = "/workspace/stillwind.config.ts";
      const files = new Map([
        [entry, 'import { settings } from "./settings.js"; export default async () => settings;'],
        [
          "/workspace/settings.ts",
          'import defaults from "./defaults.json"; import { css } from "preset"; export const settings = { ...defaults, css } satisfies Record<string, unknown>;',
        ],
        ["/workspace/defaults.json", '{"files":["*.tsx"]}'],
        [
          "/workspace/node_modules/preset/package.json",
          '{"exports":{"node":"./node.js","browser":"./browser.js"}}',
        ],
        ["/workspace/node_modules/preset/node.js", 'throw new Error("Node export executed");'],
        ["/workspace/node_modules/preset/browser.js", 'export const css = "first.css";'],
      ]);
      const evaluate = () =>
        new Promise<{ value?: unknown; error?: string }>((resolve, reject) => {
          worker.onmessage = (event) => resolve(event.data);
          worker.onerror = (event) => reject(new Error(event.message));
          worker.postMessage({
            files: [...files],
            entry,
          });
        });
      try {
        const first = await evaluate();
        files.set("/workspace/node_modules/preset/browser.js", 'export const css = "second.css";');
        const second = await evaluate();
        const rejected: string[] = [];
        for (const source of ['import "node:fs";', 'import "./missing.ts";']) {
          files.set(entry, source);
          rejected.push((await evaluate()).error ?? "");
        }
        return {
          first: first.value ?? first.error,
          second: second.value ?? second.error,
          rejected,
        };
      } finally {
        worker.terminate();
      }
    });
    expect(result.first).toEqual({
      files: ["*.tsx"],
      css: "first.css",
    });
    expect(result.second).toEqual({
      files: ["*.tsx"],
      css: "second.css",
    });
    expect(result.rejected[0]).toContain("Node module is unavailable in the browser: node:fs");
    expect(result.rejected[1]).toContain("Cannot resolve import ./missing.ts");
    expect(errors).toEqual([]);
  } finally {
    await browser?.close();
    await new Promise<void>((resolve, reject) =>
      server.close((error) => (error ? reject(error) : resolve())),
    );
  }
});
