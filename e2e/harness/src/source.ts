import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname } from "node:path";

import { fixturePath } from "./workspace.ts";

/**
 * Captures the listed files and returns a restorer that writes only changed content.
 * Paths are relative to directory and must stay inside it. reset accepts paths to retain,
 * so a production theme can keep its contents and modification time across warm builds.
 * Rejects invalid paths and propagates file errors. Files outside the list are unchanged.
 */
export async function snapshotSources(directory: string, files: readonly string[]) {
  const originals = new Map<string, Buffer>();
  for (const file of files) {
    originals.set(file, await readFile(fixturePath(directory, file)));
  }
  return {
    async reset(retain: readonly string[] = []): Promise<void> {
      for (const [file, original] of originals) {
        if (retain.includes(file)) {
          continue;
        }
        const path = fixturePath(directory, file);
        const current = await readFile(path).catch((error: NodeJS.ErrnoException) => {
          if (error.code === "ENOENT") {
            return undefined;
          }
          throw error;
        });
        if (!current?.equals(original)) {
          await mkdir(dirname(path), { recursive: true });
          await writeFile(path, original);
        }
      }
    },
  };
}
