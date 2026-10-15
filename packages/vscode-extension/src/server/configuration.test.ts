import { expect, test, vi } from "vite-plus/test";

import { bundleConfiguration } from "./configuration.ts";

vi.mock("@rolldown/browser", () => import("rolldown"));

test("browser configurations bundle captured TypeScript, JSON, and browser package exports on each reload", async () => {
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
    const module = await import(
      `data:text/javascript;base64,${Buffer.from(code).toString("base64")}`
    );
    return module.default();
  };
  expect(await evaluate()).toEqual({
    files: ["*.tsx"],
    css: "first.css",
  });
  files.set("/workspace/node_modules/preset/browser.js", 'export const css = "second.css";');
  expect(await evaluate()).toEqual({
    files: ["*.tsx"],
    css: "second.css",
  });
});

test.each([
  ['import "node:fs";', "Node module is unavailable in browser configuration: node:fs"],
  ['import "./missing.ts";', "Cannot resolve configuration import ./missing.ts"],
])("browser configurations reject unavailable imports", async (source, message) => {
  const entry = "/workspace/stillwind.config.ts";
  await expect(bundleConfiguration(new Map([[entry, source]]), entry)).rejects.toThrow(message);
});
