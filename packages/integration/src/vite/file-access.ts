import { readFile, realpath, stat } from "node:fs/promises";
import { isAbsolute } from "node:path";

import { cleanModuleId } from "@stillwind/host";
import { isFileServingAllowed, type ViteDevServer } from "vite";

/**
 * Checks both the requested and canonical file before a plugin reads it.
 *
 * @param id - Absolute filesystem identity, optionally with a query or fragment removed for access checks.
 * @param server - Optional development server whose file-serving policy applies to both paths.
 * @returns The canonical absolute path after resolving symlinks.
 * @throws If the identity is invalid, either path is denied, or realpath fails.
 */
export async function readableFile(id: string, server?: ViteDevServer): Promise<string> {
  const file = cleanModuleId(id);
  if (!isAbsolute(file) || file.includes("\0")) {
    throw new Error("Stillwind requires an absolute file path");
  }
  if (server && !isFileServingAllowed(file, server)) {
    throw new Error("Stillwind file access denied");
  }
  const canonical = await realpath(file);
  if (server && !isFileServingAllowed(canonical, server)) {
    throw new Error("Stillwind file access denied");
  }
  return canonical;
}

/**
 * Reads a file after a watcher reports a change to it.
 *
 * A writer that truncates a file before it writes the new text can cause the change event between
 * the two steps, and the watcher drops the event of the write that follows. An empty read
 * therefore waits up to 100 ms for the modification time to change and then reads the file again,
 * as Vite does when it reads a file for a hot update.
 *
 * @param file - Absolute path of the changed file.
 * @returns The UTF-8 text of the file. The text is empty when the file stays empty after the wait.
 * @throws If reading or checking the file fails.
 */
export async function readChangedFile(file: string): Promise<string> {
  const text = await readFile(file, "utf8");
  if (text) {
    return text;
  }
  const modified = (await stat(file)).mtimeMs;
  for (let attempt = 0; attempt < 10; attempt++) {
    await new Promise((resolve) => setTimeout(resolve, 10));
    if ((await stat(file)).mtimeMs !== modified) {
      break;
    }
  }
  return readFile(file, "utf8");
}
