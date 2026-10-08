/** Compiler settings, validation, and default normalization for build and analysis hosts. */

import type { NativeIdentifierNaming } from "@stillwind/native";

import {
  resolveLintConfig,
  validateLintConfig,
  type LintConfig,
  type ResolvedLintRules,
} from "./lint.ts";
import { keys, record, requireValue, text } from "./validation.ts";

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

/** Controls document CSS delivery through build integrations. */
export interface BuildOptions {
  /** Applies producer CSS to the document. Defaults to true. */
  automatic?: boolean;
}

/**
 * Compiler and design-system settings shared by build integrations and analysis services.
 */
export interface BaseConfig {
  /** Lint presets and rule overrides. Omission uses recommended; an empty object disables all rules. */
  lint?: LintConfig;
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

/** Compiler and document CSS delivery settings shared by build adapters. */
export interface BuildConfig extends BaseConfig, BuildOptions {}

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
    validateLintConfig(value.lint, `${location}.lint`, partial);
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
  lint: { rules: ResolvedLintRules };
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

  const lint = resolveLintConfig(config.lint);

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

export const compilerKeys = [
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
