/**
 * Shared settings for build integrations and analysis hosts. Use `defineConfig` for
 * stillwind.config.ts exports, and `resolveConfig` to evaluate and validate them.
 * Node configuration discovery and file evaluation are provided by `@stillwind/config/node`.
 */

import type { NativeIdentifierNaming } from "@stillwind/native";
import type { PreparedSource, ProjectInput } from "@stillwind/projection";

/**
 * Compiler and design-system settings shared by build integrations and analysis services.
 */
export interface BaseConfig {
  /** Lint reporting and safe source normalization. CSS severities apply before these overrides. */
  lint?: LintOptions;
  /** Targets for explicit Stillwind sorting and the optional sorting diagnostic. */
  sort?: UtilitySortOptions;
  /**
   * Project directory, absolute or relative to the configuration file's directory.
   * An inline override uses the application's build root instead. Defaults to that base directory.
   */
  root?: string;
  /**
   * TypeScript configuration path, absolute or relative to the adapter project root. Omission
   * selects an existing root `tsconfig.json`. False uses isolated checker defaults. Semantic
   * resolution uses these same settings. Backends capture them when created. Recreate the backend
   * to apply changed settings.
   */
  tsconfig?: string | false;
  /** Apply producer CSS to the document. Disable for libraries that supply explicit CSS values. */
  automatic?: boolean;
  /** Design-system stylesheet alias used when css is absent. Paths are absolute or relative to the project root. */
  theme?: string;
  /** Design-system stylesheet, absolute or relative to the project root. Takes precedence over `theme`. */
  css?: string;
  /** Namespace for Stillwind-owned CSS identifiers. Defaults to --sw-. */
  prefix?: string;
  /** Namespace for Tailwind-compatible internal custom properties. Defaults to --tw-. */
  tailwindPrefix?: string;
  /**
   * Stable namespace included in generated class identities. Use the same value for
   * client and server builds. Changing it changes those identities. Defaults to empty.
   */
  classNamespace?: string;
  /**
   * Emit native CSS if() for experimental values and variants. False uses fallback helpers.
   * Defaults to false. Comparisons, negated variants, and CSS-wide helper branches require true.
   */
  useCssIf?: boolean;
  /**
   * Host name generators, one optional function per identifier family. A function
   * returns the name to write, or `undefined` to keep the compiler's own name. A
   * generator must be pure: the same request has to produce the same name. Keep the
   * functions stable for the life of the session, and update `namingRevision` with them.
   */
  naming?: NativeIdentifierNaming;
  /**
   * Caller-supplied revision of naming callbacks, included in artifact cache keys. Update it when
   * callbacks change. The compiler does not derive it from functions. Defaults to an empty string.
   */
  namingRevision?: string;
  /**
   * Number of base-36 hash digits kept in built-in generated names. Defaults to 5.
   * Integer values from 1 to 50 are accepted. Raising it lowers the chance of a collision.
   */
  hashLength?: number;
}

/** Reporting policy for a lint rule. Off disables its diagnostics and automatic fixes. */
export type LintSeverity = "warn" | "deny" | "off";

/** Settings for canonical utility spelling and compiler ordering. */
export type LintOptions = {
  /** Defaults to warn, with spacing conversion enabled at 16 pixels per rem. */
  "canonical-utilities"?: LintSeverity | CanonicalUtilitiesOptions;
  /** Enables an optional sorting diagnostic. Omission disables it unless CSS enables it. */
  "sort-utilities"?: LintSeverity;
};

/** Controls replacement of equivalent utility spellings. */
export type CanonicalUtilitiesOptions = {
  /** Overrides the stylesheet severity. Omission retains it, or warn when unset. */
  severity?: LintSeverity;
  /** Converts pixel spacing to rem when the active spacing scale uses rem. Defaults to true. */
  pxToRem?: boolean;
  /** Positive finite number of pixels per rem. Defaults to 16. Used only for spacing conversion. */
  rootFontSize?: number;
};

/** Controls input grouping and independent utility and attribute ordering. */
export type UtilitySortOptions = {
  /** Sorts utilities within each static string or template segment. Defaults to true. */
  stringLiterals?: boolean;
  /**
   * Groups arrays, argument lists, and object entries by input kind. Defaults to true.
   * Each ref's marker precedes its projections; bases, utilities, custom properties,
   * and attributes follow. Each group retains its original order.
   */
  arrays?: boolean;
  /** Sorts aria-/data- names within adjacent attribute entries. Defaults to false. */
  attributes?: boolean;
};

/** One project's compiler settings, analysis scope, and source preparation operations. */
export interface AnalysisProjectConfig extends BaseConfig {
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

/** Build overrides and the selection of a configuration file and project. */
export interface InlineConfig extends BaseConfig {
  /**
   * Configuration file path, absolute or relative to the application root. An explicit path must
   * exist. Omission searches upward for stillwind.config.ts. False disables file loading.
   */
  configFile?: string | false;
  /**
   * Exact configured project name. Omission selects the sole project or the unique project whose
   * root most closely contains the application root. Ambiguous selection fails during evaluation.
   */
  project?: string;
}

/**
 * Merges objects recursively. Undefined preserves previous values, arrays are replaced, and a
 * changed provider kind replaces that provider's complete configuration.
 *
 * @param base - Settings to preserve where overrides omit a value.
 * @param overrides - Fields to merge or replace. Neither input is modified.
 * @returns Merged settings. Unchanged values and replacement arrays can share input references.
 */
export function mergeConfig<T extends object>(base: T, overrides: Partial<T>): T {
  return mergeValue(base, overrides) as T;
}

function mergeValue(base: unknown, override: unknown): unknown {
  if (override === undefined) {
    return base;
  }
  if (
    !object(base) ||
    !object(override) ||
    (override.kind !== undefined && base.kind !== override.kind)
  ) {
    return override;
  }
  const result = { ...base };
  for (const [key, value] of Object.entries(override)) {
    if (value !== undefined) {
      result[key] = mergeValue(base[key], value);
    }
  }
  return result;
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

/**
 * Checks configuration structure, compiler settings, and provider callbacks without calling them.
 *
 * @param value - Evaluated configuration to check. Unknown top-level and project fields are rejected.
 * @returns The same settings object after validation. Paths and globs are not resolved here.
 * @throws If projects, names, settings, or provider callbacks do not satisfy the configuration schema.
 */
export function validateConfig(value: unknown): StillwindConfig {
  if (
    !object(value) ||
    Object.keys(value).some((key) => key !== "projects") ||
    !Array.isArray(value.projects) ||
    !value.projects.length
  ) {
    throw new Error("Stillwind configuration must contain only a nonempty projects array");
  }
  const names = new Set<string>();
  for (const project of value.projects) {
    if (
      !object(project) ||
      Object.keys(project).some((key) => !projectKeys.includes(key)) ||
      !strings(project.files) ||
      !project.files.length ||
      (project.name !== undefined && !text(project.name)) ||
      (project.exclude !== undefined && !strings(project.exclude)) ||
      !text(project.css ?? project.theme) ||
      !validCompilerSettings(project)
    ) {
      throw new Error(
        "Each Stillwind project must specify CSS, nonempty files globs, and valid compiler settings",
      );
    }
    if (project.name !== undefined) {
      if (names.has(project.name as string)) {
        throw new Error(`Duplicate Stillwind project name: ${project.name}`);
      }
      names.add(project.name as string);
    }
    if (project.sources !== undefined && !validSourceProvider(project.sources)) {
      throw new Error("Source providers must select filesystem or custom source operations");
    }
    if (project.projection !== undefined && !validProjectionProvider(project.projection)) {
      throw new Error(
        "Projection providers must select framework converters or a custom prepare operation",
      );
    }
    if (project.naming !== undefined && !validNaming(project.naming)) {
      throw new Error("Naming settings must contain identifier generator functions");
    }
  }
  return value as unknown as StillwindConfig;
}

function validCompilerSettings(project: Record<string, unknown>): boolean {
  if (project.lint !== undefined && !validLintOptions(project.lint)) {
    return false;
  }
  if (
    project.sort !== undefined &&
    (!object(project.sort) ||
      !Object.entries(project.sort).every(
        ([key, value]) =>
          ["stringLiterals", "arrays", "attributes"].includes(key) &&
          (value === undefined || typeof value === "boolean"),
      ))
  ) {
    return false;
  }
  const stringSettings = [
    "root",
    "css",
    "theme",
    "prefix",
    "tailwindPrefix",
    "classNamespace",
    "namingRevision",
  ];
  if (
    !stringSettings.every((key) => project[key] === undefined || typeof project[key] === "string")
  ) {
    return false;
  }
  if (project.tsconfig !== undefined && project.tsconfig !== false && !text(project.tsconfig)) {
    return false;
  }
  for (const key of ["automatic", "useCssIf"]) {
    if (project[key] !== undefined && typeof project[key] !== "boolean") {
      return false;
    }
  }
  if (project.typeFacts !== undefined && project.typeFacts !== false) {
    return false;
  }
  const length = project.hashLength;
  return (
    length === undefined ||
    (typeof length === "number" && Number.isInteger(length) && length >= 1 && length <= 50)
  );
}

function validSourceProvider(value: unknown): boolean {
  if (!object(value)) {
    return false;
  }
  switch (value.kind) {
    case "filesystem":
      return Object.keys(value).every((key) => key === "kind");
    case "custom":
      return (
        typeof value.supports === "function" &&
        typeof value.load === "function" &&
        (value.resolve === undefined || typeof value.resolve === "function")
      );
    default:
      return false;
  }
}

function validProjectionProvider(value: unknown): boolean {
  if (!object(value)) {
    return false;
  }
  switch (value.kind) {
    case "framework":
      return Object.keys(value).every((key) => key === "kind");
    case "custom":
      return typeof value.prepare === "function";
    default:
      return false;
  }
}

function validNaming(value: unknown): boolean {
  return (
    object(value) &&
    Object.entries(value).every(
      ([key, fn]) => namingKeys.includes(key) && (fn === undefined || typeof fn === "function"),
    )
  );
}

const projectKeys = [
  "lint",
  "sort",
  "name",
  "root",
  "tsconfig",
  "automatic",
  "theme",
  "css",
  "prefix",
  "tailwindPrefix",
  "classNamespace",
  "useCssIf",
  "naming",
  "namingRevision",
  "hashLength",
  "files",
  "exclude",
  "sources",
  "projection",
  "typeFacts",
];

function validLintOptions(value: unknown): boolean {
  if (!object(value)) {
    return false;
  }
  const severity = (value: unknown) =>
    typeof value === "string" && ["warn", "deny", "off"].includes(value);
  return Object.entries(value).every(([rule, setting]) => {
    const fields =
      rule === "canonical-utilities" ? ["severity", "pxToRem", "rootFontSize"] : undefined;
    if (setting === undefined && (fields || rule === "sort-utilities")) {
      return true;
    }
    if (rule === "sort-utilities") {
      return severity(setting);
    }
    if (!fields) {
      return false;
    }
    if (typeof setting === "string") {
      return severity(setting);
    }
    return (
      object(setting) &&
      Object.entries(setting).every(
        ([key, option]) =>
          fields.includes(key) &&
          (option === undefined ||
            (key === "severity"
              ? severity(option)
              : key === "rootFontSize"
                ? typeof option === "number" && Number.isFinite(option) && option > 0
                : typeof option === "boolean")),
      )
    );
  });
}
const namingKeys = [
  "utilityClass",
  "refMarker",
  "anchorName",
  "containerName",
  "keyframesName",
  "viewTransitionClass",
  "viewTransitionSlot",
  "conditionHelper",
  "layerName",
];
const object = (value: unknown): value is Record<string, unknown> =>
  value !== null && typeof value === "object" && !Array.isArray(value);
const text = (value: unknown): value is string =>
  typeof value === "string" && Boolean(value.trim());
const strings = (value: unknown): value is string[] => Array.isArray(value) && value.every(text);
