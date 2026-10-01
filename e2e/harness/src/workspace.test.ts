import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { expect, test } from "vite-plus/test";

import { createWorkspace, editFile, fixturePath } from "./workspace.ts";

test("copies application files, excludes artifacts, and edits only its disposable copy", async () => {
  const root = await mkdtemp(join(tmpdir(), "stillwind-e2e-"));
  try {
    await writeFile(join(root, "package.json"), "{}");
    await writeFile(join(root, "source.ts"), "export const color = 'red';\r\n");
    await writeFile(
      join(root, "tsconfig.json"),
      JSON.stringify({ extends: "../tsconfig.base.json" }),
    );
    await mkdir(join(root, "dist"));
    await writeFile(join(root, "dist", "old.js"), "old");
    const workspace = await createWorkspace(root);
    try {
      const config = JSON.parse(await readFile(join(workspace.directory, "tsconfig.json"), "utf8"));
      expect(join(workspace.directory, config.extends)).toBe(join(root, "../tsconfig.base.json"));
      await expect(readFile(join(workspace.directory, "dist", "old.js"))).rejects.toMatchObject({
        code: "ENOENT",
      });
      const restore = await editFile(workspace.directory, {
        file: "source.ts",
        from: "'red'",
        to: "'blue'",
      });
      expect(await readFile(join(workspace.directory, "source.ts"), "utf8")).toContain("'blue'");
      expect(await readFile(join(root, "source.ts"), "utf8")).toBe(
        "export const color = 'red';\r\n",
      );
      await restore();
      expect(await readFile(join(workspace.directory, "source.ts"))).toEqual(
        await readFile(join(root, "source.ts")),
      );
      await expect(
        editFile(workspace.directory, { file: "source.ts", from: "missing", to: "blue" }),
      ).rejects.toThrow("one occurrence");
      await writeFile(join(workspace.directory, "source.ts"), "red red");
      await expect(
        editFile(workspace.directory, { file: "source.ts", from: "red", to: "blue" }),
      ).rejects.toThrow("one occurrence");
      expect(await readFile(join(workspace.directory, "source.ts"), "utf8")).toBe("red red");
    } finally {
      await workspace.dispose();
    }
    await expect(readFile(join(workspace.directory, "package.json"))).rejects.toMatchObject({
      code: "ENOENT",
    });
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("rejects paths outside the fixture before reading or writing them", () => {
  const root = join(tmpdir(), "fixture");
  expect(() => fixturePath(root, "../outside.ts")).toThrow("escapes fixture");
  expect(() => fixturePath(root, join(tmpdir(), "outside.ts"))).toThrow("escapes fixture");
  expect(fixturePath(root, "src/App.tsx")).toBe(join(root, "src", "App.tsx"));
});
