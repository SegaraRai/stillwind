import { expect, test, vi } from "vite-plus/test";

import { bundleModule } from "./bundle.ts";
import { browserPluginHost } from "./plugins.ts";

vi.mock("@rolldown/browser", () => import("rolldown"));

/**
 * Evaluates a bundled module through a data URL, which Node accepts in place of a Blob URL.
 * @param code - ESM source.
 * @returns The module namespace.
 */
const evaluate = (code: string) =>
  import(`data:text/javascript;base64,${Buffer.from(code).toString("base64")}`);

test("plugin packages are bundled from the inventory again only after a bundled file changes", async () => {
  const entry = "/workspace/node_modules/demo/index.js";
  const files = new Map([
    [
      "/workspace/node_modules/demo/package.json",
      '{"name":"demo","exports":{".":"./index.js","./data.json":"./data.json"}}',
    ],
    [entry, 'import { name } from "./name.js"; export default { name, css: () => "" };'],
    ["/workspace/node_modules/demo/name.js", 'export const name = "demo";'],
    ["/workspace/node_modules/demo/data.json", "{}"],
  ]);
  const bundle = vi.fn(bundleModule);
  const host = browserPluginHost(files, bundle, evaluate);
  const importer = "/workspace/stillwind.css";
  const watched = new Set<string>();
  const watch = (file: string) => watched.add(file);

  const plugin = await host.load("demo", importer, watch);
  expect(plugin).toMatchObject({ name: "demo" });
  expect(watched).toEqual(new Set([entry, "/workspace/node_modules/demo/name.js"]));
  expect(await host.load("demo", importer, watch)).toBe(plugin);
  expect(bundle).toHaveBeenCalledTimes(1);
  files.set("/workspace/node_modules/demo/name.js", 'export const name = "imported";');
  expect(await host.load("demo", importer, watch)).toMatchObject({
    name: "imported",
  });
  files.set(entry, 'export default { name: "changed", css: () => "" };');
  expect(await host.load("demo", importer, watch)).toMatchObject({
    name: "changed",
  });

  expect(await host.resolve("demo/data.json", importer)).toBe(
    "/workspace/node_modules/demo/data.json",
  );
  expect(await host.read("/workspace/node_modules/demo/data.json")).toBe("{}");
  expect(await host.resolve("missing/data.json", importer)).toBe(undefined);
  await expect(host.read("/workspace/missing.json")).rejects.toThrow("does not exist");
  await expect(host.load("missing", importer, watch)).rejects.toThrow(
    "Cannot resolve Stillwind plugin",
  );
});

test("a module without a plugin default export is rejected and evaluated again later", async () => {
  const entry = "/workspace/node_modules/broken/index.js";
  const files = new Map([
    ["/workspace/node_modules/broken/package.json", '{"exports":"./index.js"}'],
    [entry, "export default {};"],
  ]);
  const bundle = vi.fn(bundleModule);
  const host = browserPluginHost(files, bundle, evaluate);

  await expect(host.load("broken", "/workspace/stillwind.css", () => {})).rejects.toThrow(
    "must export a plugin object by default",
  );
  await expect(host.load("broken", "/workspace/stillwind.css", () => {})).rejects.toThrow();
  expect(bundle).toHaveBeenCalledTimes(2);
});
