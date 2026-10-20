/** Compiler settings, validation, and default normalization for build and analysis hosts. */

import type { NativeIdentifierNaming } from "@stillwind/native";
import * as v from "valibot";

import {
  resolveLintConfig,
  validateLintConfig,
  type LintConfig,
  type ResolvedLintRules,
} from "./lint.ts";
import type { StillwindPlugin } from "./plugins.ts";
import { nonemptyText, strictObject, validate } from "./validation.ts";

/** Controls independent sorting targets in scripts and stylesheets. */
export type UtilitySortOptions = {
  /** Sorts utilities within each static string or template segment. Defaults to true. */
  stringLiterals?: boolean;
  /**
   * Groups arrays, argument lists, and object entries by input kind. Defaults to true.
   * Each ref's marker precedes its projections. Bases, utilities, and custom
   * properties follow. Each group retains its original order.
   */
  arrays?: boolean;
  /** Sorts utilities within each CSS `@apply` statement. Defaults to true. */
  applyUtilities?: boolean;
  /** Orders adjacent CSS `@apply` statements while preserving competing writes. Defaults to false. */
  applyStatements?: boolean;
  /** Orders adjacent applied CSS `@variant` blocks while preserving competing writes. Defaults to false. */
  variantBlocks?: boolean;
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
  /**
   * Lint presets and rule overrides. Omission uses recommended.
   * False or an empty object disables lint rules and fixes while retaining compiler diagnostics.
   */
  lint?: LintConfig;
  /** Targets for Stillwind sorting. False disables explicit sorting and its diagnostics and fixes. */
  sort?: UtilitySortOptions | false;
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
  /**
   * Orders utilities by authored definition positions or Tailwind property metadata.
   * Defaults to authored.
   */
  utilityOrder?: "authored" | "tailwind";
  /**
   * Minimum browser versions, such as ["chrome111", "firefox113", "safari17"].
   * A single string is accepted. An empty list keeps color fallbacks.
   * Omission uses adapter defaults, or keeps fallbacks when the host supplies none.
   * Removes generated compatibility branches only when every target supports the value.
   */
  target?: string | readonly string[];
  /**
   * Namespace for Stillwind-owned CSS identifiers.
   * Starts with --, ends with -, and contains no whitespace.
   * Defaults to --sw-.
   */
  prefix?: string;
  /**
   * Namespace for Tailwind-compatible custom properties.
   * Uses the same format as prefix and must differ from it.
   * Defaults to --tw-.
   */
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
  /**
   * Plugins that `@plugin` statements of the design-system stylesheet select by name. An entry
   * takes precedence over a package of the same name. A plugin without a matching statement is
   * not loaded.
   */
  plugins?: readonly StillwindPlugin[];
  /**
   * Positive finite number of pixels per rem in the documents that use the CSS. Defaults to 16.
   * Canonical lint converts pixel spacing with it, and editor previews annotate rem values with it.
   * Media and container query annotations always use 16 because browsers evaluate rem there
   * against the initial font size.
   */
  rootFontSize?: number;
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

const sortOptionsSchema = strictObject({
  stringLiterals: v.optional(v.boolean()),
  arrays: v.optional(v.boolean()),
  applyUtilities: v.optional(v.boolean()),
  applyStatements: v.optional(v.boolean()),
  variantBlocks: v.optional(v.boolean()),
});

const browserTarget = v.pipe(
  v.string(),
  v.regex(
    /^(?:android|chrome|edge|firefox|ie|ios|opera|safari|samsung)\d+(?:\.\d+){0,2}$/,
    "must name a browser followed by one to three decimal version components",
  ),
  v.check(
    (value) =>
      value
        .replace(/^[a-z]+/, "")
        .split(".")
        .every((part) => Number(part) <= 65535),
    "version components must not exceed 65535",
  ),
);

/** Shared schemas for compiler fields, excluding delivery options and analysis scope. Lint is validated separately. */
export const compilerEntries = {
  root: v.optional(v.string()),
  css: v.optional(cssPath),
  tsconfig: v.optional(v.union([nonemptyText, v.literal(false)])),
  prefix: v.optional(namespace),
  tailwindPrefix: v.optional(namespace),
  classNamespace: v.optional(v.string()),
  utilityOrder: v.optional(v.picklist(["authored", "tailwind"])),
  target: v.optional(v.union([browserTarget, v.array(browserTarget)])),
  useCssIf: v.optional(v.boolean()),
  namingRevision: v.optional(v.string()),
  hashLength: v.optional(v.pipe(v.number(), v.integer(), v.minValue(1), v.maxValue(50))),
  rootFontSize: v.optional(
    v.pipe(
      v.number(),
      v.finite(),
      v.check((value) => value > 0, "must be positive and finite"),
    ),
  ),
  sort: v.optional(v.lazy((value) => (value === false ? v.literal(false) : sortOptionsSchema))),
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
  plugins: v.optional(
    v.array(
      v.object({
        name: nonemptyText,
        css: v.function(),
        resolve: v.optional(v.function()),
        catalog: v.optional(v.function()),
      }),
    ),
  ),
  lint: v.optional(v.unknown()),
};

const compilerSchema = strictObject(compilerEntries);

/**
 * Checks namespace collisions after deciding whether defaults can be applied.
 * @param partial - Whether to defer the check when either namespace is absent from an override.
 * @returns A schema check that attributes collisions to `tailwindPrefix`. Complete settings use
 * the default namespace for each omitted field.
 */
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

/**
 * Validates compiler fields and lint policies without resolving paths or invoking naming callbacks.
 * @param value - Compiler settings with no analysis or delivery fields. CSS is optional here.
 * @param location - Field path prepended to validation errors. Defaults to `config`.
 * @param partial - Whether incomplete overrides can defer namespace collisions and required lint
 * options until they are merged with project settings.
 * @throws If a field is unknown or invalid, namespaces collide, or lint presets or effective options are invalid.
 */
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
  /** Validated Stillwind namespace, defaulting to --sw-. */
  prefix: string;
  /** Validated Tailwind namespace, defaulting to --tw- and distinct from prefix. */
  tailwindPrefix: string;
  /** Namespace shared by client and server class identities, defaulting to an empty string. */
  classNamespace: string;
  /** Ordering policy after applying the authored-order default. */
  utilityOrder: "authored" | "tailwind";
  /** Captured minimum browser versions. An empty list keeps color fallbacks. */
  target: string[];
  /** Whether native CSS if() features are enabled. Defaults to false. */
  useCssIf: boolean;
  /** Caller-supplied revision of naming callbacks, defaulting to an empty string. */
  namingRevision: string;
  /** Number of base-36 digits in generated hashes, between 1 and 50 inclusive. Defaults to 5. */
  hashLength: number;
  /** Positive finite number of pixels per rem. Defaults to 16. */
  rootFontSize: number;
  /** Explicit flags for every sorting target. A disabled sort setting makes every flag false. */
  sort: Required<UtilitySortOptions>;
  /** Expanded lint settings with a severity and default detection options for every rule. */
  lint: {
    /** Complete rules after ordered preset composition. Unconfigured rules are off. */
    rules: ResolvedLintRules;
  };
  /** Whether producer CSS applies to the document. Defaults to true. */
  automatic: boolean;
}

/**
 * Validates compiler fields and applies defaults without resolving paths.
 * @param config - Compiler and delivery settings. Other fields are discarded before validation.
 * @returns A new object containing compiler and delivery settings with explicit sort flags and
 * expanded lint rules. Naming callbacks and other unchanged objects can retain input references.
 * @throws If compiler fields, delivery settings, namespace combinations, or effective lint policies are invalid.
 */
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
    target: typeof config.target === "string" ? [config.target] : [...(config.target ?? [])],
    useCssIf: config.useCssIf ?? false,
    namingRevision: config.namingRevision ?? "",
    hashLength: config.hashLength ?? 5,
    rootFontSize: config.rootFontSize ?? 16,
    automatic: automatic ?? true,
    sort:
      config.sort === false
        ? {
            stringLiterals: false,
            arrays: false,
            applyUtilities: false,
            applyStatements: false,
            variantBlocks: false,
          }
        : {
            stringLiterals: config.sort?.stringLiterals ?? true,
            arrays: config.sort?.arrays ?? true,
            applyUtilities: config.sort?.applyUtilities ?? true,
            applyStatements: config.sort?.applyStatements ?? false,
            variantBlocks: config.sort?.variantBlocks ?? false,
          },
    lint,
  };
}

/** Compiler field names used to discard analysis and delivery fields before compiler validation. */
export const compilerKeys = Object.keys(compilerEntries) as (keyof BaseConfig)[];
