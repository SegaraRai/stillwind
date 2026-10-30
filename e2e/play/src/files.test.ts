import { readdir, readFile } from "node:fs/promises";
import { resolve } from "node:path";

import { chromium, type Browser } from "playwright";
import { expect, test } from "vite-plus/test";

import { servePlayground } from "./server.ts";

test("the application worker requests imported JSON, declarations and CSS without reading unused files", async () => {
  const directory =
    process.env.STILLWIND_PLAY_TEST_DIRECTORY ??
    resolve(import.meta.dirname, "../../../apps/play/dist/acceptance");
  const assets = await Promise.all(
    (await readdir(resolve(directory, "_astro")))
      .filter((name) => name.endsWith(".js"))
      .map(async (name) => ({
        name,
        source: await readFile(resolve(directory, "_astro", name), "utf8"),
      })),
  );
  const entry = assets.find(
    ({ source }) => source.includes("browser-workspace") && source.includes("StillwindApplication"),
  );
  expect(entry).toBeDefined();
  const server = await servePlayground(directory);
  let browser: Browser | undefined;
  try {
    browser = await chromium.launch({
      channel: "chrome",
      headless: true,
    });
    const page = await browser.newPage();
    await page.route(`${server.origin}/fixture`, (route) =>
      route.fulfill({
        contentType: "text/html",
        body: "<!doctype html><title>Worker files</title>",
        headers: {
          "Cross-Origin-Opener-Policy": "same-origin",
          "Cross-Origin-Embedder-Policy": "require-corp",
        },
      }),
    );
    await page.goto(`${server.origin}/fixture`);
    const result = await page.evaluate(
      async ({ url }) => {
        const files = new Map([
          [
            "/workspace/App.tsx",
            'import data from "demo"; import "./empty.d.ts"; import "./app.css"; export const count = data.value.length;',
          ],
          ["/workspace/node_modules/demo/package.json", '{"exports":"./data.json"}'],
          [
            "/workspace/node_modules/demo/data.json",
            JSON.stringify({
              value: "x".repeat(120_000),
            }),
          ],
          ["/workspace/empty.d.ts", ""],
          ["/workspace/app.css", '@import "./base.css"; main {display:block}'],
          ["/workspace/base.css", "main {color:rgb(1,2,3)}"],
          [
            "/workspace/unused.json",
            JSON.stringify({
              value: "unused".repeat(100_000),
            }),
          ],
        ]);
        const reads: string[] = [];
        const worker = new Worker(url, {
          type: "module",
        });
        try {
          const output = await new Promise<{
            code: string;
            css: string;
          }>((resolve, reject) => {
            const timer = setTimeout(
              () => reject(new Error("Application worker did not finish")),
              90_000,
            );
            worker.onerror = (event) => {
              clearTimeout(timer);
              reject(new Error(event.message));
            };
            worker.onmessage = ({ data }) => {
              if (data.kind === "read") {
                const path = decodeURIComponent(new URL(data.uri).pathname);
                reads.push(path);
                worker.postMessage({
                  ...data,
                  kind: "read-reply",
                  value: files.get(path),
                });
              } else if (data.kind === "compile") {
                worker.postMessage({
                  ...data,
                  kind: "compile-reply",
                  value: {
                    graph: {
                      generation: "complete",
                      modules: [],
                      batches: [],
                    },
                    css: "",
                    rejectedNames: [],
                    cssValues: [],
                  },
                });
              } else {
                clearTimeout(timer);
                if (data.kind === "failed") {
                  reject(new Error(data.error));
                } else {
                  resolve(data);
                }
              }
            };
            worker.postMessage({
              kind: "build",
              id: 1,
              generation: 1,
              revision: 1,
              files: [...files].map(([path, text]) => [
                path,
                path.endsWith("/package.json") ? text : null,
              ]),
            });
          });
          return {
            code: output.code,
            css: output.css,
            reads,
          };
        } finally {
          worker.terminate();
        }
      },
      { url: `${server.origin}/_astro/${entry!.name}` },
    );
    await page.addScriptTag({ content: result.code });
    expect(
      await page.evaluate(
        () =>
          (
            globalThis as typeof globalThis & {
              StillwindApplication: {
                count: number;
              };
            }
          ).StillwindApplication.count,
      ),
    ).toBe(120_000);
    expect(result.css).toContain("main {color:rgb(1,2,3)}");
    expect(result.css).toContain("main {display:block}");
    expect(result.reads.toSorted()).toEqual(
      [
        "/workspace/App.tsx",
        "/workspace/app.css",
        "/workspace/base.css",
        "/workspace/empty.d.ts",
        "/workspace/node_modules/demo/data.json",
      ].toSorted(),
    );
  } finally {
    await browser?.close();
    await server.close();
  }
});
