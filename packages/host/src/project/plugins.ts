import { readFile, readdir, stat } from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";
import { pathToFileURL } from "node:url";

import { evaluateModule } from "@stillwind/config/node";

import { completePluginPaths, type PluginDirectoryEntry } from "../css/plugin-paths.ts";
import { defaultPluginExport, type LoadedPlugin, type PluginHost } from "../css/plugins.ts";

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
 * Plugin files evaluated from a relative or absolute `@plugin` path, keyed by entry path. Each entry
 * records the text of its bundled local files, so a load reuses the plugin only while every text is
 * unchanged.
 */
const files = new Map<
  string,
  {
    /** Text of each bundled local file when the plugin was evaluated. */
    sources: ReadonlyMap<string, string>;
    /** Files that the evaluation observed, including resolved packages. */
    dependencies: readonly string[];
    /** Plugin that the entry exports by default. */
    plugin: LoadedPlugin;
  }
>();

/**
 * Loads plugin packages and plugin files with Node module resolution from the stylesheet that
 * contains the `@plugin` statement, and reads files from the filesystem.
 *
 * A package specifier is imported with Node's module cache, so a changed package takes effect only
 * in a new process. A relative or absolute path is bundled with its local imports and evaluated
 * again after any of those files changes.
 */
export const nodePluginHost: PluginHost = {
  complete: (prefix, importer, root) =>
    completePluginPaths(prefix, importer, root, {
      read: (id) => readFile(id, "utf8"),
      directory: readPluginDirectory,
    }),
  async load(specifier, importer, watch) {
    const resolved = createRequire(importer).resolve(specifier);
    if (!specifier.startsWith(".") && !path.isAbsolute(specifier)) {
      return defaultPluginExport(await import(pathToFileURL(resolved).href), specifier);
    }
    let cached = files.get(resolved);
    if (
      !cached ||
      !(await Promise.all(
        [...cached.sources].map(async ([file, text]) =>
          readFile(file, "utf8").then(
            (current) => current === text,
            () => false,
          ),
        ),
      ).then((results) => results.every(Boolean)))
    ) {
      const { exported, dependencies, sources } = await evaluateModule(resolved);
      cached = {
        sources,
        dependencies,
        plugin: defaultPluginExport(
          {
            default: exported,
          },
          specifier,
        ),
      };
      files.set(resolved, cached);
    }
    for (const dependency of cached.dependencies) {
      watch(dependency);
    }
    return cached.plugin;
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
