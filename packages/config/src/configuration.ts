/** Configuration exports and analysis projects, validated before paths or providers are evaluated. */

import type { PreparedSource, ProjectInput } from "@stillwind/projection";
import * as v from "valibot";

import {
  compilerEntries,
  validateCompilerConfig,
  type BaseConfig,
  type BuildOptions,
} from "./compiler.ts";
import {
  nonemptyStringList,
  nonemptyStrings,
  nonemptyText,
  object,
  strictObject,
  validate,
} from "./validation.ts";

/** One project's compiler settings, analysis scope, and source preparation operations. */
export interface AnalysisProjectConfig extends BaseConfig, BuildOptions {
  /** Nonempty design-system stylesheet path, absolute or relative to root. Authored whitespace is preserved. */
  css: string;
  /** Optional nonempty identifier used to select this project. Names must be unique, and authored whitespace is preserved. */
  name?: string;
  /**
   * Nonempty list of globs relative to root. Analysis enumerates these targets and filters reports
   * to their scope. Imported dependencies and global declarations can lie outside this scope.
   */
  files: readonly string[];
  /** Globs relative to root that remove analysis targets. node_modules and .git are always excluded. */
  exclude?: readonly string[];
  /** Omission reads files and prepares supported framework documents. */
  sources?: SourceProvider;
  /** Omission uses built-in framework converters. Custom preparation applies to unprepared inputs only. */
  projection?: ProjectionProvider;
  /** False skips checker queries while retaining locally determined diagnostics. */
  typeFacts?: false;
}

type Captured<T> = T extends (...arguments_: never[]) => unknown
  ? T
  : T extends object
    ? {
        readonly [K in keyof T]: Captured<T[K]>;
      }
    : T;

/** Project settings fixed for a service's lifetime, with provider functions preserved. */
export type CapturedProjectConfig = Captured<AnalysisProjectConfig>;

const capturedProjects = new WeakSet<object>();

/**
 * Captures configuration data and callback registries without changing the caller's objects.
 * @param config - Evaluated project settings. Provider methods retain their original receivers.
 * @returns Fixed settings. A previously captured configuration is shared without copying it again.
 */
export function captureProjectConfig(config: AnalysisProjectConfig): CapturedProjectConfig {
  if (capturedProjects.has(config)) {
    return config;
  }

  const { naming, sources, projection, ...settings } = config;
  const data = structuredClone(settings);

  const capturedSources =
    sources?.kind === "custom"
      ? {
          kind: sources.kind,
          supports: sources.supports.bind(sources),
          load: sources.load.bind(sources),
          ...(sources.resolve ? { resolve: sources.resolve.bind(sources) } : {}),
        }
      : sources && { kind: sources.kind };
  const capturedProjection =
    projection?.kind === "custom"
      ? { kind: projection.kind, prepare: projection.prepare.bind(projection) }
      : projection && { kind: projection.kind };

  const captured: CapturedProjectConfig = {
    ...data,
    ...(naming
      ? {
          naming: {
            utilityClass: naming.utilityClass,
            refMarker: naming.refMarker,
            anchorName: naming.anchorName,
            containerName: naming.containerName,
            keyframesName: naming.keyframesName,
            viewTransitionClass: naming.viewTransitionClass,
            viewTransitionSlot: naming.viewTransitionSlot,
            conditionHelper: naming.conditionHelper,
            layerName: naming.layerName,
          },
        }
      : {}),
    ...(capturedSources ? { sources: capturedSources } : {}),
    ...(capturedProjection ? { projection: capturedProjection } : {}),
  };
  capturedProjects.add(captured);
  return captured;
}

/** Supplies original or prepared sources without starting a bundler. */
export type SourceProvider =
  | {
      /** Reads original files through the analysis host, including captured unsaved buffers. */
      kind: "filesystem";
    }
  | {
      /** Delegates source support, loading, and optional dependency metadata to the caller. */
      kind: "custom";
      /**
       * Determines whether the host can analyze a resolved source.
       *
       * @param id - Full source identity used for loading and dependency traversal.
       * @returns Whether this provider supplies analyzable source for the identity.
       */
      supports(id: string): boolean;
      /**
       * Supplies source before or after preparation, preserving the requested resolver identity.
       *
       * @param id - Resolved source identity. The returned resolverId must equal this value.
       * @param context - Project root and buffers captured for this analysis. Consult these buffers
       * when loading unsaved documents.
       * @returns Original input for preparation, or prepared input with checker correspondences.
       * @throws If the source cannot be loaded or prepared.
       */
      load(id: string, context: SourceProviderContext): Promise<ProjectInput | PreparedSource>;
      /**
       * Supplies a source implementation resolution that takes precedence over checker resolution.
       *
       * @param specifier - Authored import request.
       * @param importer - Resolved identity of the importing source.
       * @param context - Root and captured buffers for the same analysis.
       * @returns A source identity, or null when this provider cannot resolve the request.
       * @throws If resolution fails for reasons other than an unknown request.
       */
      resolve?(
        specifier: string,
        importer: string,
        context: SourceProviderContext,
      ): Promise<string | null>;
    };

/** Captured buffers shared by source reads, projections, and semantic queries. */
export interface SourceProviderContext {
  /** Absolute project directory used for relative compiler and checker identities. */
  root: string;
  /** Original source text keyed by absolute file path, captured before asynchronous analysis. */
  documents: ReadonlyMap<string, string>;
}

/** Selects framework converters or a caller-supplied preparation operation. */
export type ProjectionProvider =
  | {
      /** Uses the built-in converter selected by the input's framework metadata. */
      kind: "framework";
    }
  | {
      /** Delegates preparation of original inputs to the caller. */
      kind: "custom";
      /**
       * Prepares original source for scanning and checker queries.
       *
       * @param input - Source input with its resolver, compiler, checker, and document identities.
       * @returns Prepared source preserving those identities and original source correspondences.
       * @throws If conversion or validation of source correspondences fails.
       */
      prepare(input: ProjectInput): Promise<PreparedSource>;
    };

/** Configuration file settings evaluated by build integrations and analysis hosts. */
export interface StillwindConfig {
  /** Nonempty project list. Projects keep independent scopes and design systems. Supplied names must be unique. */
  projects: AnalysisProjectConfig[];
}

/** Called without arguments during configuration evaluation. Returns settings directly or asynchronously. */
export type StillwindConfigFactory = () => StillwindConfig | Promise<StillwindConfig>;

/** Configuration module export accepted by Node hosts and build configuration consumers. */
export type StillwindConfigExport =
  | StillwindConfig
  | Promise<StillwindConfig>
  | StillwindConfigFactory;

/**
 * Provides contextual types and returns the supplied export unchanged. Factories run and promises
 * are awaited when the consumer calls resolveConfig. Definition does not validate their results.
 *
 * @param config - Settings, a promise for settings, or a factory returning either form.
 * @returns The same object, promise, or function with its inferred type preserved.
 */
export function defineConfig<T extends StillwindConfigExport>(config: T): T {
  return config;
}

/**
 * Awaits a configuration export, calls a factory once without arguments when supplied, and
 * validates the resulting settings. Factory exceptions and rejected promises propagate.
 *
 * @param value - Settings, a promise, or a factory from a configuration module's default export.
 * @returns The validated settings object without copying it.
 * @throws If evaluation fails or the resulting settings are invalid.
 */
export async function resolveConfig(value: unknown): Promise<StillwindConfig> {
  const config = await value;
  return validateConfig(typeof config === "function" ? await config() : config);
}

const projectSchema = strictObject({
  ...compilerEntries,
  css: v.nonOptional(compilerEntries.css, "must specify CSS as a nonempty path"),
  automatic: v.optional(v.boolean()),
  name: v.optional(nonemptyText),
  files: nonemptyStringList,
  exclude: v.optional(nonemptyStrings),
  typeFacts: v.optional(v.literal(false)),
  sources: v.optional(
    v.pipe(
      v.custom<Record<string, unknown>>(object, "must be an object"),
      v.variant("kind", [
        v.strictObject({
          kind: v.literal("filesystem"),
        }),
        v.strictObject({
          kind: v.literal("custom"),
          supports: v.function(),
          load: v.function(),
          resolve: v.optional(v.function()),
        }),
      ]),
    ),
  ),
  projection: v.optional(
    v.pipe(
      v.custom<Record<string, unknown>>(object, "must be an object"),
      v.variant("kind", [
        v.strictObject({
          kind: v.literal("framework"),
        }),
        v.strictObject({
          kind: v.literal("custom"),
          prepare: v.function(),
        }),
      ]),
    ),
  ),
});

const configurationSchema = v.pipe(
  strictObject({
    projects: v.pipe(v.array(projectSchema), v.minLength(1, "must be a nonempty projects array")),
  }),
  v.rawCheck(({ dataset, addIssue }) => {
    if (!dataset.typed) {
      return;
    }
    const names = new Set<string>();
    for (const [index, project] of dataset.value.projects.entries()) {
      if (project.name === undefined) {
        continue;
      }
      if (names.has(project.name)) {
        addIssue({
          message: `Duplicate Stillwind project name: ${project.name}`,
          path: [
            {
              type: "object",
              origin: "value",
              input: dataset.value,
              key: "projects",
              value: dataset.value.projects,
            },
            {
              type: "array",
              origin: "value",
              input: dataset.value.projects,
              key: index,
              value: project,
            },
            {
              type: "object",
              origin: "value",
              input: project,
              key: "name",
              value: project.name,
            },
          ],
        });
      }
      names.add(project.name);
    }
  }),
);

/**
 * Validates evaluated projects and their compiler settings without running providers or resolving paths.
 * @param value - Evaluated settings with a nonempty project list, required CSS paths and file globs,
 * and unique names when names are supplied.
 * @returns The supplied settings with object and callback identities preserved.
 * @throws If a field, project name, effective lint policy, or compiler namespace is invalid.
 * Validation errors include the authored field path.
 */
export function validateConfig(value: unknown): StillwindConfig {
  validate(configurationSchema, value, "config");

  for (const [index, project] of value.projects.entries()) {
    const {
      automatic: _automatic,
      name: _name,
      files: _files,
      exclude: _exclude,
      sources: _sources,
      projection: _projection,
      typeFacts: _typeFacts,
      ...compiler
    } = project;
    validateCompilerConfig(compiler, `projects[${index}]`);
  }

  return value as StillwindConfig;
}
