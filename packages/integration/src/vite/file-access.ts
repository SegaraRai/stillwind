import { realpath } from "node:fs/promises";
import { isAbsolute } from "node:path";

import { cleanModuleId } from "@stillwind/host";
import { isFileServingAllowed, type ViteDevServer } from "vite";

/** Checks both the requested and canonical file before a plugin reads it. */
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
