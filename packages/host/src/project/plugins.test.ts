import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";

import { afterEach, beforeEach, expect, test } from "vite-plus/test";

import { loadNodePlugin } from "./plugins.ts";

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
        exports: "./index.js",
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

  expect((await loadNodePlugin("valid", importer)).name).toBe("valid");
  await expect(loadNodePlugin("invalid", importer)).rejects.toThrow(
    "must export a plugin object by default",
  );
  await expect(loadNodePlugin("missing", importer)).rejects.toThrow();
});
