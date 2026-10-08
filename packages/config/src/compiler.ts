/** Compiler settings, validation, and default normalization for build and analysis hosts. */

import type { NativeIdentifierNaming } from "@stillwind/native";
import * as v from "valibot";

import {
  resolveLintConfig,
  validateLintConfig,
  type LintConfig,
  type ResolvedLintRules,
} from "./lint.ts";
import { nonemptyText, strictObject, validate } from "./validation.ts";

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

const cssPath = v.pipe(
  v.string("must specify CSS as a nonempty path"),
  v.check((value) => Boolean(value.trim()), "must specify CSS as a nonempty path"),
);

const namespace = v.pipe(
  v.string(),
  v.regex(/^--\S+-$/, "must start with --, end with -, and contain no whitespace"),
);

export const compilerEntries = {
  root: v.optional(v.string()),
  css: v.optional(cssPath),
  tsconfig: v.optional(v.union([nonemptyText, v.literal(false)])),
  prefix: v.optional(namespace),
  tailwindPrefix: v.optional(namespace),
  classNamespace: v.optional(v.string()),
  utilityOrder: v.optional(v.picklist(["authored", "tailwind"])),
  useCssIf: v.optional(v.boolean()),
  namingRevision: v.optional(v.string()),
  hashLength: v.optional(v.pipe(v.number(), v.integer(), v.minValue(1), v.maxValue(50))),
  sort: v.optional(
    strictObject({
      stringLiterals: v.optional(v.boolean()),
      arrays: v.optional(v.boolean()),
      attributes: v.optional(v.boolean()),
    }),
  ),
  naming: v.optional(
    strictObject({
      utilityClass: v.optional(v.function()),
      refMarker: v.optional(v.function()),
      anchorName: v.optional(v.function()),
      containerName: v.optional(v.function()),
      keyframesName: v.optional(v.function()),
      viewTransitionClass: v.optional(v.function()),
      viewTransitionSlot: v.optional(v.function()),
      conditionHelper: v.optional(v.function()),
      layerName: v.optional(v.function()),
    }),
  ),
  lint: v.optional(v.unknown()),
};

const compilerSchema = strictObject(compilerEntries);

const distinctNamespaces = (partial: boolean) =>
  v.forward(
    v.check(
      (value: v.InferOutput<typeof compilerSchema>) =>
        (partial && (value.prefix === undefined || value.tailwindPrefix === undefined)) ||
        (value.prefix ?? "--sw-") !== (value.tailwindPrefix ?? "--tw-"),
      "must differ from prefix",
    ),
    ["tailwindPrefix"],
  );

/** Validates compiler settings independently of analysis scope. CSS is optional here. Partial overrides defer namespace collisions until merging. Throws with the invalid field's path. */
export function validateCompilerConfig(
  value: unknown,
  location = "config",
  partial = false,
): asserts value is BaseConfig {
  validate(v.pipe(compilerSchema, distinctNamespaces(partial)), value, location);
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
    validate(v.boolean(), automatic, "config.automatic");
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

export const compilerKeys = Object.keys(compilerEntries) as (keyof BaseConfig)[];
