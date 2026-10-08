/** Configuration exports and analysis projects, validated before paths or providers are evaluated. */

import type { PreparedSource, ProjectInput } from "@stillwind/projection";

import {
  compilerKeys,
  validateCompilerConfig,
  type BaseConfig,
  type BuildOptions,
} from "./compiler.ts";
import { keys, record, requireValue, stringArray, text } from "./validation.ts";

/** One project's compiler settings, analysis scope, and source preparation operations. */
export interface AnalysisProjectConfig extends BaseConfig, BuildOptions {
  /** Required design-system stylesheet, absolute or relative to root. */
  css: string;
  /** Optional identifier used to select this project from a build integration. Names must be unique. */
  name?: string;
  /**
   * Nonempty list of globs relative to root. Analysis enumerates these targets and filters reports
   * to their scope. Imported dependencies and global declarations can lie outside this scope.
   */
  files: string[];
  /** Globs relative to root that remove analysis targets. node_modules and .git are always excluded. */
  exclude?: string[];
  /** Omission reads files and prepares supported framework documents. */
  sources?: SourceProvider;
  /** Omission uses built-in framework converters. Custom preparation applies to unprepared inputs only. */
  projection?: ProjectionProvider;
  /** False skips checker queries while retaining locally determined diagnostics. */
  typeFacts?: false;
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

/** Validates an evaluated configuration without calling providers or resolving paths. Throws with the invalid field's path. */
export function validateConfig(value: unknown): StillwindConfig {
  record(value, "config");
  keys(value, ["projects"], "config");
  nonemptyArray(value.projects, "projects");
  const names = new Set<string>();
  for (const [index, project] of value.projects.entries()) {
    const location = `projects[${index}]`;
    record(project, location);
    keys(
      project,
      [
        ...compilerKeys,
        "automatic",
        "name",
        "files",
        "exclude",
        "sources",
        "projection",
        "typeFacts",
      ],
      location,
    );
    const { automatic, name, files, exclude, sources, projection, typeFacts, ...compiler } =
      project;
    validateCompilerConfig(compiler, location);
    requireValue(text(project.css), `${location}.css`, "must specify CSS as a nonempty path");
    stringArray(files, `${location}.files`, true);
    if (exclude !== undefined) {
      stringArray(exclude, `${location}.exclude`);
    }
    if (automatic !== undefined) {
      requireValue(typeof automatic === "boolean", `${location}.automatic`, "must be boolean");
    }
    if (name !== undefined) {
      requireValue(text(name), `${location}.name`, "must be a nonempty name");
      requireValue(
        !names.has(name),
        `${location}.name`,
        `Duplicate Stillwind project name: ${name}`,
      );
      names.add(name);
    }
    if (typeFacts !== undefined) {
      requireValue(typeFacts === false, `${location}.typeFacts`, "must be false");
    }
    if (sources !== undefined) {
      record(sources, `${location}.sources`);
      if (sources.kind === "filesystem") {
        keys(sources, ["kind"], `${location}.sources`);
      } else {
        requireValue(
          sources.kind === "custom",
          `${location}.sources.kind`,
          "must be filesystem or custom",
        );
        keys(sources, ["kind", "supports", "load", "resolve"], `${location}.sources`);
        requireValue(
          typeof sources.supports === "function",
          `${location}.sources.supports`,
          "must be a function",
        );
        requireValue(
          typeof sources.load === "function",
          `${location}.sources.load`,
          "must be a function",
        );
        if (sources.resolve !== undefined) {
          requireValue(
            typeof sources.resolve === "function",
            `${location}.sources.resolve`,
            "must be a function",
          );
        }
      }
    }
    if (projection !== undefined) {
      record(projection, `${location}.projection`);
      if (projection.kind === "framework") {
        keys(projection, ["kind"], `${location}.projection`);
      } else {
        requireValue(
          projection.kind === "custom",
          `${location}.projection.kind`,
          "must be framework or custom",
        );
        keys(projection, ["kind", "prepare"], `${location}.projection`);
        requireValue(
          typeof projection.prepare === "function",
          `${location}.projection.prepare`,
          "must be a function",
        );
      }
    }
  }
  return value as unknown as StillwindConfig;
}

function nonemptyArray(value: unknown, location: string): asserts value is unknown[] {
  requireValue(
    Array.isArray(value) && value.length > 0,
    location,
    "must be a nonempty projects array",
  );
}
