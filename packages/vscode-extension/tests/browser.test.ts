import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { extname, resolve, sep } from "node:path";

import { chromium, type Browser } from "playwright";
import { expect, test } from "vite-plus/test";

test("the packaged browser bundler evaluates and reloads virtual configurations with its WASM workers", async () => {
  const directory = resolve(import.meta.dirname, "../dist/extension/dist/browser");
  const server = createServer(async (request, response) => {
    const pathname = new URL(request.url!, "http://localhost").pathname;
    response.setHeader("Cross-Origin-Opener-Policy", "same-origin");
    response.setHeader("Cross-Origin-Embedder-Policy", "require-corp");
    if (pathname === "/") {
      response
        .writeHead(200, { "Content-Type": "text/html" })
        .end("<!doctype html><title>Browser configuration</title>");
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
    browser = await chromium.launch({ channel: "chrome", headless: true });
    const page = await browser.newPage();
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    await page.goto(`http://127.0.0.1:${address.port}/`);
    // Vitest rewrites imports before Playwright serializes callbacks.
    await page.addScriptTag({
      type: "module",
      content: "globalThis.__stillwindImport = (url) => import(url);",
    });
    const result = await page.evaluate(async () => {
      const importModule = (
        globalThis as typeof globalThis & {
          __stillwindImport: (
            url: string,
          ) => Promise<
            typeof import("../src/server/configuration.ts") & { default(): Promise<unknown> }
          >;
        }
      ).__stillwindImport;
      const url = "/configuration.js";
      const { bundleConfiguration } = await importModule(url);
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
      const evaluate = async () => {
        const code = await bundleConfiguration(files, entry);
        const blob = URL.createObjectURL(new Blob([code], { type: "text/javascript" }));
        try {
          const module = await importModule(blob);
          return await module.default();
        } finally {
          URL.revokeObjectURL(blob);
        }
      };
      const first = await evaluate();
      files.set("/workspace/node_modules/preset/browser.js", 'export const css = "second.css";');
      const second = await evaluate();
      const rejected: string[] = [];
      for (const source of ['import "node:fs";', 'import "./missing.ts";']) {
        files.set(entry, source);
        try {
          await bundleConfiguration(files, entry);
          rejected.push("");
        } catch (error) {
          rejected.push(String(error));
        }
      }
      return {
        first,
        second,
        rejected,
      };
    });
    expect(result.first).toEqual({
      files: ["*.tsx"],
      css: "first.css",
    });
    expect(result.second).toEqual({
      files: ["*.tsx"],
      css: "second.css",
    });
    expect(result.rejected[0]).toContain(
      "Node module is unavailable in browser configuration: node:fs",
    );
    expect(result.rejected[1]).toContain("Cannot resolve configuration import ./missing.ts");
    expect(errors).toEqual([]);
  } finally {
    await browser?.close();
    await new Promise<void>((resolve, reject) =>
      server.close((error) => (error ? reject(error) : resolve())),
    );
  }
});
