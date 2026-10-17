import { readFile, readdir, stat } from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";
import { pathToFileURL } from "node:url";

import { completePluginPaths, type PluginDirectoryEntry } from "../css/plugin-paths.ts";
import { defaultPluginExport, type PluginHost } from "../css/plugins.ts";

/**
 * Lists disk children for plugin completion, following directory symlinks.
 * @param id - Absolute directory to enumerate without loading its scripts.
 * @returns Immediate entries in filesystem order. Broken symlinks are treated as files.
 * @throws If the directory cannot be read.
 */
export async function readPluginDirectory(id: string): Promise<PluginDirectoryEntry[]> {
  return Promise.all(
    (await readdir(id, { withFileTypes: true })).map(async (entry) => ({
      name: entry.name,
      directory:
        entry.isDirectory() ||
        (entry.isSymbolicLink() &&
          (await stat(path.join(id, entry.name))
            .then((value) => value.isDirectory())
            .catch(() => false))),
    })),
  );
}

/**
 * Loads plugin packages and plugin files with Node module resolution from the stylesheet that
 * contains the `@plugin` statement, and reads files from the filesystem.
 */
export const nodePluginHost: PluginHost = {
  complete: (prefix, importer, root) =>
    completePluginPaths(prefix, importer, root, {
      read: (id) => readFile(id, "utf8"),
      directory: readPluginDirectory,
    }),
  async load(specifier, importer) {
    const resolved = createRequire(importer).resolve(specifier);
    return defaultPluginExport(await import(pathToFileURL(resolved).href), specifier);
  },
  async resolve(specifier, importer) {
    try {
      return createRequire(importer).resolve(specifier);
    } catch {
      return undefined;
    }
  },
  async read(path) {
    return await readFile(path, "utf8");
  },
};
