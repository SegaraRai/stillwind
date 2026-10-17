import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";

import { afterEach, beforeEach, expect, test } from "vite-plus/test";

import { nodePluginHost } from "./plugins.ts";

let root: string;

beforeEach(async () => {
  root = await mkdtemp(path.join(tmpdir(), "stillwind-plugin-"));
  for (const [name, source] of [
    ["valid", 'export default { name: "valid", css: () => "" };'],
    ["invalid", "export default { css: () => '' };"],
  ]) {
    await mkdir(path.join(root, "node_modules", name), { recursive: true });
    await writeFile(
      path.join(root, "node_modules", name, "package.json"),
      JSON.stringify({
        name,
        type: "module",
        exports: {
          ".": "./index.js",
          "./package.json": "./package.json",
        },
      }),
    );
    await writeFile(path.join(root, "node_modules", name, "index.js"), source);
  }
});

afterEach(async () => {
  await rm(root, { recursive: true, force: true });
});

test("packages resolve from the stylesheet and must export a plugin by default", async () => {
  const importer = path.join(root, "src", "app.css");

  expect((await nodePluginHost.load("valid", importer)).name).toBe("valid");
  await expect(nodePluginHost.load("invalid", importer)).rejects.toThrow(
    "must export a plugin object by default",
  );
  await expect(nodePluginHost.load("missing", importer)).rejects.toThrow();
});

test("plugin files resolve from the stylesheet and missing requests resolve to nothing", async () => {
  const importer = path.join(root, "src", "app.css");
  const file = await nodePluginHost.resolve("valid/package.json", importer);

  expect(file).toBe(path.join(root, "node_modules", "valid", "package.json"));
  expect(JSON.parse(await nodePluginHost.read(file!)).name).toBe("valid");
  expect(await nodePluginHost.resolve("missing/package.json", importer)).toBe(undefined);
});
