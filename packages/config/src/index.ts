/**
 * Shared settings for build integrations and analysis hosts. Use `defineConfig` for
 * stillwind.config.ts exports, and `resolveConfig` to evaluate and validate them.
 * Node configuration discovery and file evaluation are provided by `@stillwind/config/node`.
 */

import { lintRules, lengthUnits, type SimpleLintRule, type LintRule } from "./lint-rules.ts";
export { lintRules, type LintRule } from "./lint-rules.ts";

import type { NativeIdentifierNaming } from "@stillwind/native";
import type { PreparedSource, ProjectInput } from "@stillwind/projection";

/**
 * Compiler and design-system settings shared by build integrations and analysis services.
 */
export interface BaseConfig {
  /** Lint reporting and safe source normalization. Omitted rules use compiler defaults. */
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
  /** Design-system stylesheet, absolute or relative to the project root.  */
  css?: string;
  /** Orders utilities by authored definition positions or Tailwind property metadata. Defaults to authored. */
  utilityOrder?: "authored" | "tailwind";
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

/** Detection and reporting policies for source utilities and compiler ordering. */
export type LintOptions = Partial<Record<SimpleLintRule, LintSeverity>> & {
  "no-arbitrary-spacing"?: LintSeverity | ArbitrarySpacingOptions;
  "no-restricted-utilities"?: LintSeverity | RestrictedUtilitiesOptions;
  "no-restricted-declarations"?: LintSeverity | RestrictedDeclarationsOptions;
  "conflicting-utilities"?: LintSeverity | ConflictingUtilitiesOptions;
  "length-units"?: LintSeverity | LengthUnitsOptions;
  "no-restricted-theme-tokens"?: LintSeverity | RestrictedThemeTokensOptions;
  "canonical-utilities"?: LintSeverity | CanonicalUtilitiesOptions;
};

/** Selects additional domains beyond margin, padding, and gap. */
export type ArbitrarySpacingOptions = {
  /** Omission uses the rule default, or warn for an explicitly configured rule. */
  severity?: LintSeverity;
  /** Case-insensitive full property globs using `*` and `?`. */
  properties?: string[];
};

/** Forbids utility spellings independently of CSS resolution. */
export type RestrictedUtilitiesOptions = {
  /** Omission uses the rule default, or warn for an explicitly configured rule. */
  severity?: LintSeverity;
  /** Full utility globs exclude enclosing variants and important modifiers. */
  utilities?: {
    pattern: string;
    message?: string;
  }[];
};

/** Forbids resolved declarations, including parsed shorthand components. */
export type RestrictedDeclarationsOptions = {
  /** Omission uses the rule default, or warn for an explicitly configured rule. */
  severity?: LintSeverity;
  /** Property and value globs ignore ASCII case. Values match complete serialized CSS text. */
  rules?: {
    properties: string[];
    values: string[];
    message?: string;
  }[];
};

/** Controls overlap between direct utility contributions. */
export type ConflictingUtilitiesOptions = {
  /** Omission uses the rule default, or warn for an explicitly configured rule. */
  severity?: LintSeverity;
  /** Allows a narrower property to override part of a shorthand. Defaults to false. */
  allowShorthandOverrides?: boolean;
};

/** Controls nonzero CSS lengths; later matching rules replace earlier restrictions. */
export type LengthUnitsOptions = {
  /** Omission uses the rule default, or warn for an explicitly configured rule. */
  severity?: LintSeverity;
  /** Property names use case-insensitive full-name globs with `*` and `?`. */
  rules?: {
    properties: string[];
    disallow: string[];
  }[];
};

/** Selects theme token names without restricting semantic aliases of those tokens. */
export type RestrictedThemeTokensOptions = {
  /** Omission uses the rule default, or warn for an explicitly configured rule. */
  severity?: LintSeverity;
  /** Case-sensitive full custom-property name globs with `*` and `?`. */
  tokens?: string[];
};

/** Controls replacement of equivalent utility spellings. */
export type CanonicalUtilitiesOptions = {
  /** Omission uses warn. */
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

/** Controls document CSS delivery through build integrations. */
export interface BuildOptions {
  /** Applies producer CSS to the document. Defaults to true. */
  automatic?: boolean;
}

/** Inline options that can pass through isolated loader workers. */
export type SerializableInlineConfig = Omit<InlineConfig, "naming">;

/** Compiler and document CSS delivery settings shared by build adapters. */
export interface BuildConfig extends BaseConfig, BuildOptions {}

/** Build overrides and the selection of a configuration file and project. */
export interface InlineConfig extends BaseConfig, BuildOptions {
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

/** Validates compiler settings independently of analysis scope. CSS is optional here. Partial overrides defer namespace collisions until merging. Throws with the invalid field's path. */
export function validateCompilerConfig(
  value: unknown,
  location = "config",
  partial = false,
): asserts value is BaseConfig {
  record(value, location);
  keys(value, compilerKeys, location);

  for (const key of [
    "root",
    "css",
    "prefix",
    "tailwindPrefix",
    "classNamespace",
    "namingRevision",
  ]) {
    if (value[key] !== undefined) {
      requireValue(typeof value[key] === "string", `${location}.${key}`, "must be a string");
    }
  }
  if (value.css !== undefined) {
    requireValue(text(value.css), `${location}.css`, "must specify CSS as a nonempty path");
  }
  if (value.tsconfig !== undefined) {
    requireValue(
      value.tsconfig === false || text(value.tsconfig),
      `${location}.tsconfig`,
      "must be a nonempty path or false",
    );
  }
  if (value.useCssIf !== undefined) {
    requireValue(typeof value.useCssIf === "boolean", `${location}.useCssIf`, "must be boolean");
  }
  if (value.utilityOrder !== undefined) {
    requireValue(
      value.utilityOrder === "authored" || value.utilityOrder === "tailwind",
      `${location}.utilityOrder`,
      "must be authored or tailwind",
    );
  }
  if (value.hashLength !== undefined) {
    requireValue(
      typeof value.hashLength === "number" &&
        Number.isInteger(value.hashLength) &&
        value.hashLength >= 1 &&
        value.hashLength <= 50,
      `${location}.hashLength`,
      "must be an integer from 1 to 50",
    );
  }
  for (const key of ["prefix", "tailwindPrefix"]) {
    if (value[key] !== undefined) {
      requireValue(
        typeof value[key] === "string" &&
          value[key].startsWith("--") &&
          value[key].endsWith("-") &&
          value[key].length >= 4 &&
          !/\s/.test(value[key]),
        `${location}.${key}`,
        "must start with --, end with -, and contain no whitespace",
      );
    }
  }
  requireValue(
    (partial && (value.prefix === undefined || value.tailwindPrefix === undefined)) ||
      (value.prefix ?? "--sw-") !== (value.tailwindPrefix ?? "--tw-"),
    `${location}.tailwindPrefix`,
    "must differ from prefix",
  );
  if (value.sort !== undefined) {
    record(value.sort, `${location}.sort`);
    keys(value.sort, ["stringLiterals", "arrays", "attributes"], `${location}.sort`);
    for (const [key, setting] of Object.entries(value.sort)) {
      if (setting !== undefined) {
        requireValue(typeof setting === "boolean", `${location}.sort.${key}`, "must be boolean");
      }
    }
  }
  if (value.naming !== undefined) {
    record(value.naming, `${location}.naming`);
    keys(value.naming, namingKeys, `${location}.naming`);
    for (const [key, setting] of Object.entries(value.naming)) {
      if (setting !== undefined) {
        requireValue(
          typeof setting === "function",
          `${location}.naming.${key}`,
          "must be a function",
        );
      }
    }
  }
  if (value.lint !== undefined) {
    validateLint(value.lint, `${location}.lint`, partial);
  }
}

function validateLint(value: unknown, location: string, partial: boolean) {
  record(value, location);
  keys(value, Object.keys(lintRules), location);
  for (const [name, setting] of Object.entries(value)) {
    if (setting === undefined) {
      continue;
    }

    const rule = lintRules[name as LintRule];
    const entry = `${location}.${name}`;
    if (
      name === "no-restricted-theme-tokens" &&
      !partial &&
      (typeof setting === "string" ? setting : object(setting) ? setting.severity : undefined) !==
        "off"
    ) {
      requireValue(
        object(setting) && Array.isArray(setting.tokens) && setting.tokens.length > 0,
        `${entry}.tokens`,
        "must contain custom-property patterns when the rule is active",
      );
    }
    if (typeof setting === "string") {
      requireValue(severity(setting), entry, "must be warn, deny, or off");
      continue;
    }

    requireValue(rule.options !== null, entry, "must be warn, deny, or off");
    record(setting, entry);
    keys(setting, rule.options, entry);
    for (const [field, option] of Object.entries(setting)) {
      if (option === undefined) {
        continue;
      }

      const path = `${entry}.${field}`;
      if (field === "severity") {
        requireValue(severity(option), path, "must be warn, deny, or off");
      } else if (field === "pxToRem" || field === "allowShorthandOverrides") {
        requireValue(typeof option === "boolean", path, "must be boolean");
      } else if (field === "rootFontSize") {
        requireValue(
          typeof option === "number" && Number.isFinite(option) && option > 0,
          path,
          "must be positive and finite",
        );
      } else if (field === "properties" || field === "tokens") {
        stringArray(option, path);
        for (const pattern of option) {
          requireValue(
            !/\s/.test(pattern) &&
              (field !== "tokens" || (pattern.startsWith("--") && pattern.length > 2)),
            path,
            "must contain valid complete globs",
          );
        }
      } else {
        requireValue(Array.isArray(option), path, "must be an array");
        for (const [index, restriction] of option.entries()) {
          const item = `${path}[${index}]`;
          record(restriction, item);
          if (field === "utilities") {
            keys(restriction, ["pattern", "message"], item);
            requireValue(
              text(restriction.pattern) && !/\s/.test(restriction.pattern),
              `${item}.pattern`,
              "must be a nonempty utility glob without whitespace",
            );
          } else {
            const units = name === "length-units";
            keys(
              restriction,
              units ? ["properties", "disallow"] : ["properties", "values", "message"],
              item,
            );
            stringArray(restriction.properties, `${item}.properties`, true);
            for (const property of restriction.properties) {
              requireValue(
                !/\s/.test(property),
                `${item}.properties`,
                "must contain complete property globs without whitespace",
              );
            }
            if (units) {
              stringArray(restriction.disallow, `${item}.disallow`);
              for (const unit of restriction.disallow) {
                requireValue(
                  lengthUnits.some((name) => name === unit.toLowerCase()),
                  `${item}.disallow`,
                  "must contain CSS length unit names",
                );
              }
            } else {
              stringArray(restriction.values, `${item}.values`, true);
            }
          }
          if (restriction.message !== undefined) {
            requireValue(
              typeof restriction.message === "string",
              `${item}.message`,
              "must be a string",
            );
          }
        }
      }
    }
  }
}

/** Compiler and delivery settings after defaults are applied. Authored paths remain unresolved. */
export interface NormalizedCompilerConfig extends BaseConfig, BuildOptions {
  prefix: string;
  tailwindPrefix: string;
  classNamespace: string;
  utilityOrder: "authored" | "tailwind";
  useCssIf: boolean;
  namingRevision: string;
  hashLength: number;
  sort: Required<UtilitySortOptions>;
  lint: LintOptions;
  automatic: boolean;
}

/** Validates compiler fields and applies defaults without resolving paths. Returns a new object containing only compiler and delivery settings. */
export function normalizeCompilerConfig(
  config: BaseConfig & BuildOptions,
): NormalizedCompilerConfig {
  const { automatic } = config;
  const compiler = Object.fromEntries(compilerKeys.map((key) => [key, config[key]]));
  validateCompilerConfig(compiler);
  if (automatic !== undefined) {
    requireValue(typeof automatic === "boolean", "config.automatic", "must be boolean");
  }

  const lint = Object.fromEntries(
    Object.entries(lintRules).flatMap(([name, rule]) => {
      const setting = config.lint?.[name as LintRule];
      const level = typeof setting === "string" ? setting : (setting?.severity ?? rule.severity);
      if (setting === undefined && level === null) {
        return [];
      }
      return [
        [
          name,
          rule.options === null
            ? level
            : {
                ...(typeof setting === "object" ? setting : {}),
                severity: level ?? "warn",
                ...(name === "canonical-utilities"
                  ? {
                      pxToRem:
                        typeof setting === "object" && "pxToRem" in setting
                          ? (setting.pxToRem ?? true)
                          : true,
                      rootFontSize:
                        typeof setting === "object" && "rootFontSize" in setting
                          ? (setting.rootFontSize ?? 16)
                          : 16,
                    }
                  : {}),
              },
        ],
      ];
    }),
  ) as LintOptions;

  return {
    ...compiler,
    prefix: config.prefix ?? "--sw-",
    tailwindPrefix: config.tailwindPrefix ?? "--tw-",
    classNamespace: config.classNamespace ?? "",
    utilityOrder: config.utilityOrder ?? "authored",
    useCssIf: config.useCssIf ?? false,
    namingRevision: config.namingRevision ?? "",
    hashLength: config.hashLength ?? 5,
    automatic: automatic ?? true,
    sort: {
      stringLiterals: config.sort?.stringLiterals ?? true,
      arrays: config.sort?.arrays ?? true,
      attributes: config.sort?.attributes ?? false,
    },
    lint,
  };
}

const compilerKeys = [
  "lint",
  "sort",
  "root",
  "tsconfig",
  "css",
  "prefix",
  "tailwindPrefix",
  "classNamespace",
  "utilityOrder",
  "useCssIf",
  "naming",
  "namingRevision",
  "hashLength",
] as const;

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

const text = (value: unknown): value is string =>
  typeof value === "string" && Boolean(value.trim());

const severity = (value: unknown) =>
  typeof value === "string" && ["warn", "deny", "off"].includes(value);

function requireValue(condition: unknown, location: string, message: string): asserts condition {
  if (!condition) {
    throw new Error(`${location}: ${message}`);
  }
}

function record(value: unknown, location: string): asserts value is Record<string, unknown> {
  requireValue(
    value !== null && typeof value === "object" && !Array.isArray(value),
    location,
    "must be an object",
  );
}

function keys(value: Record<string, unknown>, allowed: readonly string[], location: string) {
  for (const key of Object.keys(value)) {
    requireValue(allowed.includes(key), `${location}.${key}`, "unknown setting");
  }
}

function nonemptyArray(value: unknown, location: string): asserts value is unknown[] {
  requireValue(
    Array.isArray(value) && value.length > 0,
    location,
    "must be a nonempty projects array",
  );
}

function stringArray(
  value: unknown,
  location: string,
  nonempty = false,
): asserts value is string[] {
  requireValue(
    Array.isArray(value) && (!nonempty || value.length > 0) && value.every(text),
    location,
    "must be an array of nonempty strings",
  );
}

const object = (value: unknown): value is Record<string, unknown> =>
  value !== null && typeof value === "object" && !Array.isArray(value);
