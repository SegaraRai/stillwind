import { realpath } from "node:fs/promises";
import { isAbsolute } from "node:path";

import { cleanModuleId } from "@stillwind/host";
import { isFileServingAllowed, type ViteDevServer } from "vite";

/**
 * Checks both the requested and canonical file before a plugin reads it.
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
