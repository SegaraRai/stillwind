import { defaultPluginExport, type PluginHost } from "@stillwind/host/browser";
import { resolveVirtualModule } from "@stillwind/host/resolution";

/**
 * Plugin modules keyed by entry path, with the entry text that they were evaluated from. Browser
 * projects are recreated for each analysis, and bundling a plugin takes much longer than an
 * analysis. A change to a file that the entry imports is therefore not observed while the entry
 * text stays the same.
 */
const modules = new Map<
  string,
  {
    /** Entry text when the module was evaluated. */
    text: string;
    /** Plugin that the module exports by default. */
    plugin: ReturnType<PluginHost["load"]>;
  }
>();

/**
 * Creates plugin operations over a captured workspace. Plugin packages are bundled from the
 * inventory like configurations, so Node imports fail.
 *
 * @param files - Virtual absolute paths and text, including plugin packages and the files that
 * plugins read.
 * @param bundle - Bundles an entry of `files` into one ESM source.
 * @param evaluate - Evaluates an ESM source and returns its module namespace.
 * @returns Operations that resolve requests with browser package exports and read `files`.
 */
export function browserPluginHost(
  files: ReadonlyMap<string, string>,
  bundle: (files: ReadonlyMap<string, string>, entry: string) => Promise<string>,
  evaluate: (code: string) => Promise<unknown>,
): PluginHost {
  return {
    /**
     * Bundles and evaluates a plugin package, reusing the module while its entry text is
     * unchanged.
     * @param specifier - Package specifier or path from the `@plugin` statement.
     * @param importer - Stylesheet that contains the statement.
     * @returns The plugin that the module exports by default.
     * @throws If the request does not resolve, bundling or evaluation fails, or the default
     * export is not a plugin.
     */
    async load(specifier, importer) {
      const entry = resolveVirtualModule(files, specifier, importer);
      if (!entry) {
        throw new Error(`Cannot resolve Stillwind plugin ${specifier} from ${importer}`);
      }
      const text = files.get(entry)!;
      let cached = modules.get(entry);
      if (cached?.text !== text) {
        const plugin = bundle(files, entry)
          .then(evaluate)
          .then((module) => defaultPluginExport(module, specifier));
        cached = { text, plugin };
        modules.set(entry, cached);
        // A failed evaluation is retried by the next load.
        plugin.catch(() => {
          if (modules.get(entry)?.plugin === plugin) {
            modules.delete(entry);
          }
        });
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
