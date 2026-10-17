import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { pathToFileURL } from "node:url";

import { defaultPluginExport, type PluginHost } from "../css/plugins.ts";

/**
 * Loads plugin packages and plugin files with Node module resolution from the stylesheet that
 * contains the `@plugin` statement, and reads files from the filesystem.
 */
export const nodePluginHost: PluginHost = {
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
