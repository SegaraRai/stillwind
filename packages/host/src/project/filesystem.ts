import { readFile, stat } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";

import { captureProjectConfig, type AnalysisProjectConfig } from "@stillwind/config";
import { resolveProjectConfig } from "@stillwind/config/node";
import { ResolverFactory } from "oxc-resolver";

import { fileModuleDescriptor } from "../compilation/index.ts";
import { completePluginPaths, pluginDirectory } from "../css/plugin-paths.ts";
import { resolveTypeFactsEnvironment } from "../semantics/index.ts";
import type { AnalysisHost } from "./analysis.ts";
import { nodePluginHost, readPluginDirectory } from "./plugins.ts";
import { stylesheetResolver } from "./stylesheets.ts";

/**
 * Creates a Node analysis host with captured buffers, source providers, and CSS resolution.
 * Custom source resolution takes precedence over identities supplied by the semantic backend.
 *
 * @param config - One evaluated project. Custom source and projection callbacks receive the
 * buffers installed by setDocuments. Prepared inputs bypass custom projection preparation.
 * @param directory - Configuration directory used to resolve a relative project root.
 * @returns An owned host with fixed TypeScript settings. CSS and checker creation remain lazy.
 * @throws If TypeScript settings cannot be selected or the CSS resolver cannot be initialized.
 */
export function createFileSystemAnalysisHost(
  config: AnalysisProjectConfig,
  directory: string,
): AnalysisHost {
  config = captureProjectConfig(config);
  const resolvedConfig = resolveProjectConfig(config, directory);
  const root = resolvedConfig.root!;
  const typeFactsEnvironment = resolveTypeFactsEnvironment(root, resolvedConfig.tsconfig);
  const cssResolver = stylesheetResolver();
  const frameworkResolver = new ResolverFactory({
    extensions: [".astro", ".svelte", ".vue"],
    conditionNames: ["import", "default"],
    tsconfig:
      typeFactsEnvironment.project.kind === "configured"
        ? {
            configFile: typeFactsEnvironment.project.configPath,
            references: "auto",
          }
        : undefined,
  });

  let documents: ReadonlyMap<string, string> = new Map();
  let buffers: ReadonlyMap<string, string> = new Map();

  /**
   * Normalizes paths for lookup in the captured buffer map.
   * @param id - File path resolved against the process working directory when relative.
   * @returns An absolute path, lowercased on Windows to share keys across case variations.
   */
  const documentKey = (id: string) =>
    process.platform === "win32" ? path.resolve(id).toLowerCase() : path.resolve(id);

  /**
   * Reads a captured buffer before falling back to the physical file.
   * @param id - File path accepted by the Node filesystem.
   * @returns Captured text when present, otherwise UTF-8 file text.
   * @throws If the required physical file cannot be read.
   */
  const read = (id: string) =>
    Promise.resolve(buffers.get(documentKey(id)) ?? readFile(id, "utf8"));

  /**
   * Selects script and framework sources eligible for default analysis.
   * @param id - File path tested case-insensitively by extension.
   * @returns Whether the path has a supported extension and is not a TypeScript declaration file.
   */
  const supports = (id: string) =>
    /\.(?:[cm]?[jt]sx?|vue|svelte|astro)$/i.test(id) && !/\.d\.[cm]?ts$/i.test(id);

  return {
    root,
    config: resolvedConfig,
    typeFactsResolution: true,
    typeFacts: config.typeFacts,
    typeFactsEnvironment,
    supports: (id) =>
      config.sources?.kind === "custom" ? config.sources.supports(id) : supports(id),
    describe: (id) => fileModuleDescriptor(root, id),
    read,
    load: async (id) => {
      const context = { root, documents };
      const input =
        config.sources?.kind === "custom"
          ? await config.sources.load(id, context)
          : {
              resolverId: id,
              ...fileModuleDescriptor(root, id),
              document: {
                ...fileModuleDescriptor(root, id).document,
                code: await read(id),
              },
            };
      return !("kind" in input) && config.projection?.kind === "custom"
        ? config.projection.prepare(input)
        : input;
    },
    resolve: async (specifier, importer) => {
      if (config.sources?.kind === "custom") {
        const id = await config.sources.resolve?.(specifier, importer, { root, documents });
        return id
          ? {
              id,
            }
          : null;
      }
      if (/\.(?:vue|svelte|astro)$/i.test(specifier)) {
        if (specifier.startsWith(".") || path.isAbsolute(specifier)) {
          const id = path.resolve(path.dirname(importer), specifier);
          if (buffers.has(documentKey(id))) {
            return {
              id,
            };
          }
        }
        const id = (await frameworkResolver.async(path.dirname(importer), specifier)).path;
        return id
          ? {
              id,
            }
          : null;
      }
      return null;
    },
    watch: () => {},
    warn: () => {},
    css: {
      resolve: async (specifier, importer) => {
        const result = await cssResolver.async(path.dirname(importer), specifier);
        if (!result.path) {
          throw new Error(`Cannot resolve CSS ${specifier} from ${importer}: ${result.error}`);
        }
        return result.path;
      },
      read,
      documentUri: (id) => pathToFileURL(id).href,
      sourceMapSource: (id) => pathToFileURL(id).href,
      watch: () => {},
      plugins: {
        ...nodePluginHost,
        read,
        /**
         * Resolves a plugin file request, preferring open buffers for a relative or absolute
         * path so that an unsaved plugin file resolves.
         * @param specifier - Package specifier or path.
         * @param importer - Stylesheet that contains the statement.
         * @returns A buffered or existing file, or undefined when the request does not resolve
         * to a file.
         */
        async resolve(specifier, importer) {
          if (specifier.startsWith(".") || path.isAbsolute(specifier)) {
            const base = path.resolve(path.dirname(importer), specifier);
            for (const suffix of ["", ".js", ".json", "/index.js"]) {
              const candidate = base + suffix;
              if (buffers.has(documentKey(candidate))) {
                return candidate;
              }
            }
          }
          const resolved = await nodePluginHost.resolve(specifier, importer);
          return resolved &&
            (await stat(resolved)
              .then((value) => value.isFile())
              .catch(() => false))
            ? resolved
            : undefined;
        },
        complete: (prefix, importer, projectRoot) =>
          completePluginPaths(prefix, importer, projectRoot, {
            read,
            directory: async (id) => {
              const entries = [
                ...(await readPluginDirectory(id).catch(() => [])),
                ...pluginDirectory(documents, id, process.platform !== "win32"),
              ];
              return [
                ...new Map(
                  entries.map((entry) => [
                    process.platform === "win32" ? entry.name.toLowerCase() : entry.name,
                    entry,
                  ]),
                ).values(),
              ];
            },
          }),
      },
    },
    setDocuments: async (next) => {
      documents = next;
      buffers = new Map([...next].map(([id, text]) => [documentKey(id), text]));
    },
    invalidate: async () => {
      cssResolver.clearCache();
      frameworkResolver.clearCache();
    },
    close: async () => {},
  };
}
