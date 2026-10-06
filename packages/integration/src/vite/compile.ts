import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

import type { EncodedSourceMap } from "@jridgewell/gen-mapping";
import {
  cleanModuleId,
  compilerModuleId,
  fileSourceKind,
  moduleSourceMap,
  type CssUpstreamMap,
} from "@stillwind/host";
import type { Rollup } from "vite";

import type { IntegrationSession } from "../session.ts";
import type { CssContext } from "./design-system.ts";
import { finalizeModuleWithMap } from "./finalize.ts";
import { sourceModuleId } from "./module-ids.ts";
import type { ModuleRegistry } from "./module-registry.ts";
import type { IntegrationConfig, ProjectSettings } from "./project.ts";

/** Metadata retained in the Vite module graph to reconstruct a producer from a cached transform. */
export interface ProducerMetadata {
  /** Source before Stillwind compilation, absent when a projection produces no compiled code. */
  input?: string;
  /** Full Vite source identity, including any query suffix, when compilation succeeds. */
  id?: string;
  /** Compiler identity used for stylesheet selection, when compilation succeeds. */
  moduleId?: string;
  /** Whether the source imports an explicit CSS value, independently of `automatic`. */
  manual?: boolean;
  /** Whether this virtual module supplies an automatic stylesheet projection. */
  projection?: boolean;
}

/**
 * Producer source, transformed output, and dependency metadata retained for CSS planning and
 * cache recovery.
 */
export interface ProducerInput {
  /**
   * Whether the source was compiled for a stylesheet projection. Its public source name remains
   * opaque in browser CSS maps.
   */
  projection?: boolean;
  /**
   * Map from the source received by Stillwind to its original source, retained across recompilation
   * when no new map is supplied.
   */
  upstream?: CssUpstreamMap;
  /** Source text before Stillwind transformation, retained for later recompilation. */
  code: string;
  /** Map from transformed source to `code`, used when composing framework maps. */
  sourceMap?: EncodedSourceMap;
  /** Full Vite source identity, including any query suffix. */
  id: string;
  /** Compiler module identity used for stylesheet selection and composition dependencies. */
  moduleId: string;
  /**
   * Whether the producer is excluded from automatic document styles because it uses explicit CSS or
   * automatic emission is disabled.
   */
  manual: boolean;
  /**
   * Transformed source returned to Vite, used to detect CSS changes that require no JavaScript hot
   * update.
   */
  javascript: string;
  /** Compiler module identities referenced through style composition. */
  dependencies: string[];
}

/** Vite operations needed to compile producers and compose their upstream source maps. */
export interface CompileContext extends CssContext {
  /**
   * Reports a compiler warning through the current Vite operation.
   *
   * @param message - Formatted warning text.
   */
  warn: (message: string) => void;
  /**
   * Reads the map produced by plugins that transformed the source before Stillwind.
   *
   * @returns The upstream map supplied by the Vite transform context.
   */
  getCombinedSourcemap?: () => Pick<
    Rollup.SourceMap,
    "version" | "sources" | "names" | "mappings" | "sourcesContent"
  >;
}

/** A successful producer compilation, ready to record in the environment and return to Vite. */
export interface ProducerCompilation {
  /** Input retained for document planning, CSS reloads, and framework source-map composition. */
  input: ProducerInput;
  /** Transformed source and metadata returned by Vite's transform hook. */
  result: ProducerTransform;
}

/** A producer transform whose graph metadata allows recovery from cached Vite modules. */
export interface ProducerTransform {
  /** Transformed source, retaining SFC syntax or JavaScript and any automatic producer import. */
  code: string;
  /** Serialized map from transformed source to the source received by Stillwind. */
  map: string;
  /** Metadata saved in the Vite module graph across cached transforms. */
  meta: {
    /** Original source and identities used to recover a producer before production emission. */
    stillwind: Required<Pick<ProducerMetadata, "input" | "id" | "moduleId" | "manual">>;
  };
}

/**
 * Compiles a producer and prepares its generated imports, source map, and cache metadata.
 *
 * Successful compilation updates the session and registers CSS references. The caller must store
 * the returned input. SFC output keeps framework syntax, while script output is finalized and may
 * import an automatic producer module.
 *
 * @param compiler - Initialized session for the current environment.
 * @param settings - Configured root used to derive compiler and checker file identities.
 * @param config - Compiler config, including whether document CSS is automatic.
 * @param registry - Virtual reference registry for the same environment.
 * @param code - Source text before Stillwind transformation.
 * @param id - Full Vite identity of the source, including any query suffix.
 * @param context - Resolver, watcher, warning handler, and optional upstream-map reader.
 * @param previous - Previously recorded input whose projection flag and upstream map may be
 * retained.
 * @returns A promise for the recorded input and Vite transform result, or `undefined` when the
 * compiler produces no code.
 * @throws If dependency loading, compilation, or upstream-map reading fails.
 */
export async function compileProducer(
  compiler: IntegrationSession,
  settings: ProjectSettings,
  config: IntegrationConfig,
  registry: ModuleRegistry,
  code: string,
  id: string,
  context: CompileContext,
  previous?: Readonly<ProducerInput>,
): Promise<ProducerCompilation | undefined> {
  const sfc = /\.(?:astro|svelte|vue)$/.test(cleanModuleId(id));
  const documentUri = (id: string) => {
    const clean = cleanModuleId(id);
    return clean.startsWith("\0") || clean.startsWith("virtual:")
      ? `stillwind-virtual:${encodeURIComponent(clean)}`
      : pathToFileURL(clean).href;
  };

  const output = await compiler.compile(
    code,
    id,
    {
      resolve: (specifier, importer) => context.resolve(specifier, importer, { skipSelf: true }),
      read: (id) => readFile(cleanModuleId(id), "utf8"),
      describe: (resolvedId) => ({
        moduleId:
          resolvedId.startsWith("\0") || resolvedId.startsWith("virtual:")
            ? `virtual:${encodeURIComponent(resolvedId)}`
            : sourceModuleId(settings.projectRoot, resolvedId),
        document: {
          uri: documentUri(resolvedId),
          format: fileSourceKind(resolvedId).format,
        },
        checkerFileName:
          resolvedId.startsWith("\0") || resolvedId.startsWith("virtual:")
            ? resolve(
                settings.projectRoot,
                ".stillwind-checker",
                `${createHash("sha256").update(resolvedId).digest("hex")}.ts`,
              )
            : cleanModuleId(resolvedId),
        projectionFileName:
          resolvedId.startsWith("\0") || resolvedId.startsWith("virtual:")
            ? `virtual/${createHash("sha256").update(resolvedId).digest("hex")}.ts`
            : compilerModuleId(settings.projectRoot, cleanModuleId(resolvedId)),
        syntax: fileSourceKind(resolvedId).syntax,
      }),
      watch: (id) => context.addWatchFile(id),
      warn: (message) => context.warn(message),
    },
    {
      stylesheetImports:
        sfc && config.automatic !== false ? [registry.source("producer", id)] : undefined,
      moduleCssImport: registry.source("value", id),
    },
  );
  if (!output?.code) {
    return;
  }

  const finalized = sfc
    ? {
        code: output.code,
        removed: [],
        prefix: "",
      }
    : finalizeModuleWithMap(output.code, false);
  const suffix =
    sfc || output.hasModuleCssImport || config.automatic === false
      ? ""
      : `\nimport ${JSON.stringify(registry.source("producer", id))};\n`;
  const javascript = finalized.code + suffix;
  const sourceMap = moduleSourceMap(
    code,
    output.code,
    id,
    output.mappings,
    finalized.removed,
    finalized.prefix,
  );

  const input: ProducerInput = {
    sourceMap,
    projection: previous?.projection,
    upstream: context.getCombinedSourcemap
      ? {
          id: cleanModuleId(id),
          sourceMapSource: documentUri(id),
          map: JSON.stringify(context.getCombinedSourcemap()),
        }
      : previous?.upstream,
    code,
    id,
    moduleId: output.moduleId,
    manual: output.hasModuleCssImport || config.automatic === false,
    javascript,
    dependencies: output.compositionModules,
  };

  const result = {
    code: javascript,
    map: JSON.stringify(sourceMap),
    meta: {
      stillwind: {
        input: code,
        id,
        moduleId: output.moduleId,
        manual: output.hasModuleCssImport,
      },
    },
  };

  return {
    input,
    result,
  };
}
