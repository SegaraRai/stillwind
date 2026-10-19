import { defaultPluginExport, type PluginHost } from "@stillwind/host/browser";
import { resolveVirtualModule } from "@stillwind/host/resolution";

import type { BundledModule } from "./bundle.ts";

/**
 * Plugin modules keyed by entry path, with the text of each bundled file. Browser projects are
 * recreated for each analysis, and bundling a plugin takes much longer than an analysis, so a
 * load reuses the plugin while every bundled file has the same text.
 */
const modules = new Map<
  string,
  {
    /** Text of each bundled file when the module was evaluated. */
    sources: ReadonlyMap<string, string>;
    /** Plugin that the module exports by default. */
    plugin: Awaited<ReturnType<PluginHost["load"]>>;
  }
>();

/**
 * Creates plugin operations over a captured workspace. Plugin packages are bundled from the
 * inventory like configurations, so Node imports fail.
 *
 * @param files - Virtual absolute paths and text, including plugin packages and the files that
 * plugins read.
 * @param bundle - Bundles an entry of `files` into one ESM source and lists the files it contains.
 * @param evaluate - Evaluates an ESM source and returns its module namespace.
 * @returns Operations that resolve requests with browser package exports and read `files`.
 */
export function browserPluginHost(
  files: ReadonlyMap<string, string>,
  bundle: (files: ReadonlyMap<string, string>, entry: string) => Promise<BundledModule>,
  evaluate: (code: string) => Promise<unknown>,
): PluginHost {
  return {
    /**
     * Bundles and evaluates a plugin package, reusing the module while the text of every bundled
     * file is unchanged.
     * @param specifier - Package specifier or path from the `@plugin` statement.
     * @param importer - Stylesheet that contains the statement.
     * @param watch - Receives each bundled file.
     * @returns The plugin that the module exports by default.
     * @throws If the request does not resolve, bundling or evaluation fails, or the default
     * export is not a plugin.
     */
    async load(specifier, importer, watch) {
      const entry = resolveVirtualModule(files, specifier, importer);
      if (!entry) {
        throw new Error(`Cannot resolve Stillwind plugin ${specifier} from ${importer}`);
      }
      let cached = modules.get(entry);
      if (!cached || [...cached.sources].some(([file, text]) => files.get(file) !== text)) {
        // A failed bundle or evaluation stores nothing, so the next load tries again.
        const { code, modules: bundled } = await bundle(files, entry);
        cached = {
          sources: new Map(bundled.map((file) => [file, files.get(file)!])),
          plugin: defaultPluginExport(await evaluate(code), specifier),
        };
        modules.set(entry, cached);
      }
      for (const file of cached.sources.keys()) {
        watch(file);
      }
      return cached.plugin;
    },
    /**
     * Resolves a plugin file request with browser package exports.
     * @param specifier - Package specifier with an optional subpath, or a path.
     * @param importer - Stylesheet that contains the statement.
     * @returns The virtual path, or undefined when the request does not resolve.
     */
    async resolve(specifier, importer) {
      return resolveVirtualModule(files, specifier, importer);
    },
    /**
     * Reads a captured file.
     * @param path - Absolute virtual path.
     * @returns The captured text.
     * @throws If the inventory has no such file.
     */
    async read(path) {
      const text = files.get(path);
      if (text === undefined) {
        throw new Error(`Virtual file does not exist: ${path}`);
      }
      return text;
    },
  };
}
