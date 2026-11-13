import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { expect, test } from "vite-plus/test";

import { readChangedFile } from "./file-access.ts";

test("reads the text that a writer stores after it truncates the changed file", async () => {
  const root = await mkdtemp(join(tmpdir(), "stillwind-file-access-"));
  try {
    const file = join(root, "style.ts");
    await writeFile(file, "");
    const text = readChangedFile(file);
    await new Promise((resolve) => setTimeout(resolve, 30));
    await writeFile(file, 'export const shared = style("m-[23px]");\n');

    await expect(text).resolves.toBe('export const shared = style("m-[23px]");\n');
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
