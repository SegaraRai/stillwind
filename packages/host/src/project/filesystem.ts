import { readFile } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";

import { ResolverFactory } from "oxc-resolver";
import type { AnalysisProjectConfig } from "stillwind/config";

import { fileModuleDescriptor } from "../compilation/index.ts";
import { resolveTypeFactsEnvironment } from "../semantics/index.ts";
import type { AnalysisHost } from "./analysis.ts";
import { resolveProjectOptions } from "./options.ts";

/** Creates source and CSS services using the settings in stillwind.config.ts. */
export function createFileSystemAnalysisHost(
  config: AnalysisProjectConfig,
  directory: string,
): AnalysisHost {
  const options = resolveProjectOptions(config, directory);
  const root = options.root!;
  const typeFactsEnvironment = resolveTypeFactsEnvironment(root, options.tsconfig);
  const cssResolver = new ResolverFactory({
    extensions: [".css"],
    conditionNames: ["style", "import", "default"],
    mainFields: ["style", "main"],
  });

  let documents: ReadonlyMap<string, string> = new Map();
  let buffers: ReadonlyMap<string, string> = new Map();
  const documentKey = (id: string) =>
    process.platform === "win32" ? path.resolve(id).toLowerCase() : path.resolve(id);
  const read = (id: string) =>
    Promise.resolve(buffers.get(documentKey(id)) ?? readFile(id, "utf8"));
  const supports = (id: string) =>
    /\.(?:[cm]?[jt]sx?|vue|svelte|astro)$/i.test(id) && !/\.d\.[cm]?ts$/i.test(id);

  return {
    root,
    options,
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
      if (config.sources?.kind !== "custom" || !config.sources.resolve) {
        return null;
      }
      const id = await config.sources.resolve(specifier, importer, {
        root,
        documents,
      });
      return id ? { id } : null;
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
    },
    setDocuments: async (next) => {
      documents = next;
      buffers = new Map([...next].map(([id, text]) => [documentKey(id), text]));
    },
    invalidate: async () => {
      cssResolver.clearCache();
    },
    close: async () => {},
  };
}
