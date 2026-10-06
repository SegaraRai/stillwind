import type { NativeIdentifierNaming } from "@stillwind/native";
import type { PreparedSource, ProjectInput } from "@stillwind/projection";

/**
 * Settings shared by build integrations and analysis services. `css` takes precedence over
 * `theme`. Relative stylesheet paths use the selected project root.
 */
export interface StillwindOptions {
  /** Root relative to stillwind.config.ts, or to the build root when supplied directly to an integration. */
  root?: string;
  /**
   * TypeScript configuration path, absolute or relative to the adapter project root. Omission
   * selects an existing root `tsconfig.json`. False uses isolated checker defaults. Semantic
   * resolution uses these same settings. Restart the project after changing it.
   */
  tsconfig?: string | false;
  /** Apply producer CSS to the document. Disable for libraries that supply explicit CSS values. */
  automatic?: boolean;
  /** Entry stylesheet alias used when `css` is absent; absolute or project-root relative. */
  theme?: string;
  /** Entry stylesheet defining the design system; takes precedence over `theme`. */
  css?: string;
  /**
   * Destination for the theme vocabulary's module augmentation, absolute or project-root
   * relative. Defaults to `src/stillwind.generated.d.ts`; `false` disables generation. Analysis hosts keep this file in memory.
   */
  types?: false | string;
  /** Namespace for Stillwind-owned CSS identifiers. */
  prefix?: string;
  /** Namespace for Tailwind-compatible internal custom properties. */
  tailwindPrefix?: string;
  /**
   * Stable namespace included in generated class identities. Use the same value for
   * client and server builds; changing it changes those identities. Defaults to empty.
   */
  classNamespace?: string;
  /**
   * Emit native CSS if() for experimental values and variants; false uses fallback helpers.
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
  /** Hash of `naming`, used to invalidate cached artifacts when it changes. */
  namingRevision?: string;
  /**
   * Base-36 digits a built-in generated name keeps from its request hash. Defaults to 5;
   * values from 1 to 50 are accepted. Raising it lowers the chance of a collision.
   */
  hashLength?: number;
}

/** Settings shared by build integrations and each independently diagnosed project. */
export interface AnalysisProjectConfig extends StillwindOptions {
  /** Globs relative to root. Imports and global declarations are independent of this scope. */
  files: string[];
  exclude?: string[];
  /** Omission reads files and prepares supported framework documents. */
  sources?: SourceProvider;
  projection?: ProjectionProvider;
  /** False skips checker queries while retaining locally determined diagnostics. */
  typeFacts?: false;
}

/** Supplies original or prepared sources without starting a bundler. */
export type SourceProvider =
  | { kind: "filesystem" }
  | {
      kind: "custom";
      supports(id: string): boolean;
      load(id: string, context: SourceProviderContext): Promise<ProjectInput | PreparedSource>;
      /** Supplies implementation metadata absent from checker module declarations. */
      resolve?(
        specifier: string,
        importer: string,
        context: SourceProviderContext,
      ): Promise<string | null>;
    };

/** Captured buffers shared by source reads, projections, and semantic queries. */
export interface SourceProviderContext {
  root: string;
  documents: ReadonlyMap<string, string>;
}

/** Selects framework converters or a caller-supplied preparation operation. */
export type ProjectionProvider =
  | {
      kind: "framework";
    }
  | {
      kind: "custom";
      prepare(input: ProjectInput): Promise<PreparedSource>;
    };

/** Each project keeps its own scope, design system, and declaration identities. */
export interface StillwindConfig {
  projects: AnalysisProjectConfig[];
}

/** Computes project settings when a loader resolves the configuration. */
export type StillwindConfigFactory = () => StillwindConfig | Promise<StillwindConfig>;

/** Configuration module export accepted by Node hosts and build configuration consumers. */
export type StillwindConfigExport =
  | StillwindConfig
  | Promise<StillwindConfig>
  | StillwindConfigFactory;

/**
 * Provides contextual types and returns the supplied export unchanged. Factories run and promises
 * are awaited when the consumer calls resolveConfig; definition does not validate their results.
 */
export function defineConfig<T extends StillwindConfigExport>(config: T): T {
  return config;
}

/**
 * Awaits a configuration export, calls a factory once without arguments when supplied, and
 * validates the resulting settings. Factory exceptions and rejected promises propagate.
 */
export async function resolveConfig(value: unknown): Promise<StillwindConfig> {
  const config = await value;
  return validateConfig(typeof config === "function" ? await config() : config);
}

/** Validates configuration loaded from JavaScript as well as TypeScript. */
export function validateConfig(value: unknown): StillwindConfig {
  if (
    !object(value) ||
    Object.keys(value).some((key) => key !== "projects") ||
    !Array.isArray(value.projects) ||
    !value.projects.length
  ) {
    throw new Error("Stillwind configuration must contain only a nonempty projects array");
  }
  for (const project of value.projects) {
    if (
      !object(project) ||
      Object.keys(project).some((key) => !projectKeys.includes(key)) ||
      !strings(project.files) ||
      !project.files.length ||
      (project.exclude !== undefined && !strings(project.exclude)) ||
      !text(project.css ?? project.theme) ||
      !validCompilerSettings(project)
    ) {
      throw new Error(
        "Each Stillwind project must specify CSS, nonempty files globs, and valid compiler settings",
      );
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
  for (const key of ["tsconfig", "types"]) {
    const value = project[key];
    if (value !== undefined && value !== false && !text(value)) {
      return false;
    }
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
  "root",
  "tsconfig",
  "automatic",
  "theme",
  "css",
  "types",
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
