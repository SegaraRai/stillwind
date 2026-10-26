/** Ordered lint preset composition and validation for all configuration hosts. */

import * as v from "valibot";

import { lintPresets } from "./lint-presets.ts";
import { lengthUnits, lintRules, type LintRule, type SimpleLintRule } from "./lint-rules.ts";
import { nonemptyStringList, nonemptyText, object, strictObject, validate } from "./validation.ts";

/** Reporting policy for a lint rule. Off disables its diagnostics and automatic fixes. */
export type LintSeverity = "warn" | "deny" | "off";

/** Detection and reporting policies for source utilities and compiler ordering. */
export type LintRules = Partial<Record<SimpleLintRule, LintSeverity>> & {
  /** Severity or additional CSS property domains checked for arbitrary spacing. */
  "no-arbitrary-spacing"?: LintSeverity | ArbitrarySpacingOptions;
  /** Severity or patterns restricting authored utility spellings. */
  "no-restricted-utilities"?: LintSeverity | RestrictedUtilitiesOptions;
  /** Severity or restrictions on resolved CSS properties and values. */
  "no-restricted-declarations"?: LintSeverity | RestrictedDeclarationsOptions;
  /** Severity or checks of fixed implementation declarations for importance. */
  "no-important"?: LintSeverity | ImportantOptions;
  /** Severity or exceptions for a narrower property overriding a shorthand. */
  "conflicting-utilities"?: LintSeverity | ConflictingUtilitiesOptions;
  /** Severity or marker class names whose misuse is not reported. */
  "invalid-marker-use"?: LintSeverity | MarkerUseOptions;
  /** Severity or ordered restrictions on nonzero CSS length units. */
  "no-restricted-length-units"?: LintSeverity | LengthUnitsOptions;
  /** Severity or checks of fixed implementation declarations for physical properties. */
  "prefer-logical-properties"?: LintSeverity | LogicalPropertiesOptions;
  /** Severity or patterns restricting direct references to theme custom properties. */
  "no-restricted-theme-tokens"?: LintSeverity | RestrictedThemeTokensOptions;
  /** Severity and conversion settings for canonical utility replacements. */
  "prefer-canonical-utilities"?: LintSeverity | CanonicalUtilitiesOptions;
  /** Severity or length-conversion settings for `@variant` replacements in stylesheets. */
  "prefer-variant-directive"?: LintSeverity | VariantDirectiveOptions;
};

/** A named, shareable lint configuration. Extended presets apply in order before rules. */
export interface LintPreset {
  /** Optional display name used to identify this preset in configuration errors. */
  readonly name?: string;
  /** Earlier entries apply first. Omission adds no presets. Cycles are invalid. */
  readonly extends?: readonly LintPreset[];
  /** Rule overrides. Severity-only overrides preserve inherited detection options. */
  readonly rules?: LintRules;
}

/** Project lint configuration. False disables all rules. An object with omitted extends uses no presets. */
export type LintConfig = LintPreset | false;

/** Complete reporting settings with default detection options applied. */
export type ResolvedLintRules = {
  [K in keyof LintRules]-?: Exclude<LintRules[K], LintSeverity | undefined> extends never
    ? LintSeverity
    : Required<Exclude<LintRules[K], LintSeverity | undefined>> & {
        /** Effective reporting policy after preset composition and rule defaults. */
        severity: LintSeverity;
      };
};

/** Selects additional domains beyond margin, padding, and gap. */
export type ArbitrarySpacingOptions = {
  /** Omission preserves inherited severity, or uses warn for a newly configured rule. */
  severity?: LintSeverity;
  /** Complete property globs using `*` and `?`, with no whitespace and comparison that ignores ASCII case. */
  properties?: readonly string[];
};

/** Forbids utility spellings independently of CSS resolution. */
export type RestrictedUtilitiesOptions = {
  /** Omission preserves inherited severity, or uses warn for a newly configured rule. */
  severity?: LintSeverity;
  /** Restrictions on authored utility spellings. Each matching entry can supply an explanation. */
  utilities?: readonly {
    /** Nonempty, case-sensitive full utility glob without whitespace, enclosing variants, or important modifiers. */
    pattern: string;
    /** Optional project-specific diagnostic explanation, preserved without trimming. */
    message?: string;
  }[];
};

/** Forbids resolved declarations, including parsed shorthand components. */
export type RestrictedDeclarationsOptions = {
  /** Omission preserves inherited severity, or uses warn for a newly configured rule. */
  severity?: LintSeverity;
  /**
   * Include fixed `(implementation)` declarations. Omission preserves an inherited
   * setting, or uses true. Captured candidate values and unmarked extensions remain checked.
   */
  checkImplementation?: boolean;
  /** Every matching declaration restriction applies. Later entries do not override earlier entries. */
  rules?: readonly {
    /** Nonempty list of complete property globs with no whitespace. Comparison ignores ASCII case. */
    properties: readonly string[];
    /** Nonempty list of complete serialized value globs. Comparison ignores ASCII case, and whitespace, strings, and URLs remain CSS text. */
    values: readonly string[];
    /** Optional project-specific diagnostic explanation, preserved without trimming. */
    message?: string;
  }[];
};

/** Controls overlap between direct utility contributions. */
export type ConflictingUtilitiesOptions = {
  /** Omission preserves inherited severity, or uses warn for a newly configured rule. */
  severity?: LintSeverity;
  /** Allows a narrower property to override part of a shorthand. Defaults to false. */
  allowShorthandOverrides?: boolean;
};

/** Controls reports of candidates that cannot add the marker class that they name. */
export type MarkerUseOptions = {
  /** Omission preserves inherited severity, or uses warn for a newly configured rule. */
  severity?: LintSeverity;
  /** Exact decoded marker class names, such as `group` or `foo/bar`, whose findings are not reported. */
  ignoredClasses?: readonly string[];
};

/** Controls nonzero CSS lengths; later matching rules replace earlier restrictions. */
export type LengthUnitsOptions = {
  /** Omission preserves inherited severity, or uses warn for a newly configured rule. */
  severity?: LintSeverity;
  /**
   * Include fixed `(implementation)` declarations. Omission preserves an inherited
   * setting, or uses false. Captured candidate values remain checked.
   */
  checkImplementation?: boolean;
  /** Restrictions in precedence order. The last matching entry determines the forbidden units. */
  rules?: readonly {
    /** Nonempty list of complete property globs supporting `*` and `?`, without whitespace. Comparison ignores ASCII case. */
    properties: readonly string[];
    /** CSS length unit names compared without ASCII case. An empty list clears restrictions for matching properties. */
    disallow: readonly string[];
  }[];
};

/** Controls physical-property checks at utility use sites. */
export type LogicalPropertiesOptions = {
  /** Omission preserves inherited severity, or uses warn for a newly configured rule. */
  severity?: LintSeverity;
  /**
   * Include fixed `(implementation)` declarations. Omission preserves an inherited
   * setting, or uses false. Captured candidate values remain checked.
   */
  checkImplementation?: boolean;
};

/** Selects theme token names without restricting semantic aliases of those tokens. */
export type RestrictedThemeTokensOptions = {
  /** Omission preserves inherited severity, or uses warn for a newly configured rule. */
  severity?: LintSeverity;
  /**
   * Include token uses from fixed `(implementation)` declarations. Omission preserves
   * an inherited setting, or uses true. Tokens selected through placeholders or variant arguments remain checked.
   */
  checkImplementation?: boolean;
  /** Case-sensitive full custom-property name globs with `*` and `?`. */
  tokens?: readonly string[];
};

/** Controls importance restrictions on declarations and source modifiers. */
export type ImportantOptions = {
  /** Omission preserves inherited severity, or uses warn for a newly configured rule. */
  severity?: LintSeverity;
  /**
   * Include fixed `(implementation)` declarations. Omission preserves an inherited
   * setting, or uses true. Source `!` modifiers and declarations with executable placeholders remain checked.
   */
  checkImplementation?: boolean;
};

/** Controls replacement of equivalent utility spellings. */
export type CanonicalUtilitiesOptions = {
  /** Omission preserves inherited severity, or uses warn for a newly configured rule. */
  severity?: LintSeverity;
  /** Compares px and rem lengths using rootFontSize. Defaults to true. */
  pxToRem?: boolean;
  /** Combines equivalent utilities within one source container. Defaults to true. */
  collapse?: boolean;
  /** Allows logical and physical properties to share a shorthand. Defaults to false. */
  logicalToPhysical?: boolean;
  /** Keeps selector specificity. Set false to allow Tailwind's ancestor migrations. Defaults to true. */
  preserveSpecificity?: boolean;
};

/** Controls replacement of nested rules and conditional at-rules with `@variant`. */
export type VariantDirectiveOptions = {
  /** Omission preserves inherited severity, or uses warn for a newly configured rule. */
  severity?: LintSeverity;
  /**
   * Lets `px` lengths in `@media` conditions match `rem` variants at 16px = 1rem. Defaults to false.
   * `rem` in a media query follows the browser's default font size, so the replacement can switch at another width.
   */
  pxToRem?: boolean;
};

/**
 * Applies an inline override to project lint settings without expanding presets.
 * Omitted extends preserves the project's list; a supplied list replaces it.
 * Omitted project settings select recommended. Rule fields merge and arrays replace.
 * False replaces all settings. An object override of false enables only its own rules and presets.
 * Neither input is modified.
 * @param base - Project settings, or undefined to use recommended.
 * @param override - Inline settings; undefined preserves the project settings.
 * @returns False when lint remains disabled, or settings with inherited rule options preserved.
 */
export function mergeLintConfig(
  base: LintConfig | undefined,
  override: LintConfig | undefined,
): LintConfig {
  if (override === undefined) {
    return (
      base ?? {
        extends: [lintPresets.recommended],
      }
    );
  }
  if (override === false) {
    return false;
  }

  const previous: LintPreset =
    base === false
      ? {}
      : (base ?? {
          extends: [lintPresets.recommended],
        });
  return {
    ...previous,
    ...(override.name === undefined ? {} : { name: override.name }),
    ...(override.extends === undefined ? {} : { extends: override.extends }),
    rules: mergeRules(previous.rules ?? {}, override.rules ?? {}),
  };
}

/**
 * Expands presets in order and applies rule defaults without modifying the input.
 * @param config - Project lint settings. Undefined selects recommended; false or an empty object disables all rules.
 * @param location - Field path used in configuration errors.
 * @returns Every rule's severity and detection options, wrapped in rules for repeatable normalization.
 * @throws If a preset has a cycle, an unknown field, or invalid effective rule options.
 */
export function resolveLintConfig(
  config?: LintConfig,
  location = "config.lint",
): {
  /** Complete lint policies with cloned detection options and default values. */
  rules: ResolvedLintRules;
} {
  const value =
    config === false
      ? {}
      : (config ?? {
          extends: [lintPresets.recommended],
        });
  validatePreset(value, location, new Set());
  const rules = resolvedRules(value);
  validateResolvedRules(rules, `${location}.rules`);
  return { rules };
}

/**
 * Validates lint settings and preset ancestry before or after composition.
 * @param value - False to disable lint, or a preset object with optional parents and rules.
 * @param location - Field path used to identify invalid values and cyclic extends lists.
 * @param partial - Whether to defer required effective options until presets and overrides are composed.
 * @throws If preset fields or rule options are invalid, ancestry has a cycle, or complete settings
 * enable a theme restriction without token patterns.
 */
export function validateLintConfig(value: unknown, location: string, partial: boolean): void {
  if (value === false) {
    return;
  }

  validatePreset(value, location, new Set());
  if (!partial) {
    validateResolvedRules(resolvedRules(value), `${location}.rules`);
  }
}

const severitySchema = v.picklist(["warn", "deny", "off"]);

const glob = v.pipe(nonemptyText, v.regex(/^\S+$/, "must contain valid complete globs"));

const propertyGlobs = v.pipe(v.array(glob), v.minLength(1));

const tokenGlob = v.pipe(
  glob,
  v.check(
    (value) => value.startsWith("--") && value.length > 2,
    "must contain custom-property patterns",
  ),
);

const ruleOptions = {
  "no-important": strictObject({
    severity: v.optional(severitySchema),
    checkImplementation: v.optional(v.boolean()),
  }),
  "no-arbitrary-spacing": strictObject({
    severity: v.optional(severitySchema),
    properties: v.optional(v.array(glob)),
  }),
  "no-restricted-utilities": strictObject({
    severity: v.optional(severitySchema),
    utilities: v.optional(
      v.array(strictObject({ pattern: glob, message: v.optional(v.string()) })),
    ),
  }),
  "no-restricted-declarations": strictObject({
    severity: v.optional(severitySchema),
    checkImplementation: v.optional(v.boolean()),
    rules: v.optional(
      v.array(
        strictObject({
          properties: propertyGlobs,
          values: nonemptyStringList,
          message: v.optional(v.string()),
        }),
      ),
    ),
  }),
  "conflicting-utilities": strictObject({
    severity: v.optional(severitySchema),
    allowShorthandOverrides: v.optional(v.boolean()),
  }),
  "invalid-marker-use": strictObject({
    severity: v.optional(severitySchema),
    ignoredClasses: v.optional(v.array(nonemptyText)),
  }),
  "no-restricted-length-units": strictObject({
    severity: v.optional(severitySchema),
    checkImplementation: v.optional(v.boolean()),
    rules: v.optional(
      v.array(
        strictObject({
          properties: propertyGlobs,
          disallow: v.array(
            v.pipe(
              nonemptyText,
              v.check(
                (value) => lengthUnits.some((unit) => unit === value.toLowerCase()),
                "must contain CSS length unit names",
              ),
            ),
          ),
        }),
      ),
    ),
  }),
  "prefer-logical-properties": strictObject({
    severity: v.optional(severitySchema),
    checkImplementation: v.optional(v.boolean()),
  }),
  "no-restricted-theme-tokens": strictObject({
    severity: v.optional(severitySchema),
    checkImplementation: v.optional(v.boolean()),
    tokens: v.optional(v.array(tokenGlob)),
  }),
  "prefer-canonical-utilities": strictObject({
    severity: v.optional(severitySchema),
    pxToRem: v.optional(v.boolean()),
    collapse: v.optional(v.boolean()),
    logicalToPhysical: v.optional(v.boolean()),
    preserveSpecificity: v.optional(v.boolean()),
  }),
  "prefer-variant-directive": strictObject({
    severity: v.optional(severitySchema),
    pxToRem: v.optional(v.boolean()),
  }),
} satisfies Record<Exclude<LintRule, SimpleLintRule>, v.GenericSchema>;

const lintRulesSchema = strictObject(
  Object.fromEntries(
    Object.keys(lintRules).map((name) => [
      name,
      v.optional(
        name in ruleOptions
          ? v.lazy((value) =>
              typeof value === "string"
                ? severitySchema
                : ruleOptions[name as keyof typeof ruleOptions],
            )
          : severitySchema,
      ),
    ]),
  ),
);

const presetSchema = strictObject({
  name: v.optional(nonemptyText),
  extends: v.optional(v.array(v.unknown(), "must be an array of presets")),
  rules: v.optional(lintRulesSchema),
});

/**
 * Validates preset fields and rejects cycles along the current ancestry while allowing shared parents.
 * @param value - Preset object to check without modifying it or invoking callbacks.
 * @param location - Authored field path, extended with parent indexes and supplied display names.
 * @param active - Objects on the current ancestry. Success restores the previous contents.
 * A failure can leave objects from the failing ancestry in this set.
 * @throws If fields are invalid or a preset already belongs to active.
 */
function validatePreset(
  value: unknown,
  location: string,
  active: Set<object>,
): asserts value is LintPreset {
  validate(presetSchema, value, location);

  if (active.has(value)) {
    throw new Error(`${location}: cyclic lint preset extends`);
  }
  active.add(value);
  for (const [index, preset] of (value.extends ?? []).entries()) {
    const name =
      object(preset) && typeof preset.name === "string" && preset.name.trim()
        ? ` (${preset.name})`
        : "";
    validatePreset(preset, `${location}.extends[${index}]${name}`, active);
  }
  active.delete(value);
}

/**
 * Expands a validated preset and supplies reporting and detection defaults for every catalog rule.
 * @param config - Validated preset with acyclic ancestry.
 * @returns Complete rule settings. Detection options and their arrays are cloned, unconfigured
 * rules are off, and configured option objects without a severity use warn.
 * @throws If a detection option cannot be cloned.
 */
function resolvedRules(config: LintPreset): ResolvedLintRules {
  const expanded = expand(config);
  return Object.fromEntries(
    Object.entries(lintRules).map(([name, rule]) => {
      const setting = expanded[name as LintRule];
      return [
        name,
        rule.defaults === null
          ? (setting ?? "off")
          : {
              ...structuredClone(rule.defaults),
              ...(typeof setting === "object" ? structuredClone(setting) : {}),
              severity:
                typeof setting === "string"
                  ? setting
                  : (setting?.severity ?? (setting === undefined ? "off" : "warn")),
            },
      ];
    }),
  ) as ResolvedLintRules;
}

/**
 * Composes parent presets from left to right before applying the preset's own rule overrides.
 * @param config - Validated preset with acyclic ancestry. The input is not modified.
 * @returns A new rule map whose option arrays can still share references with the presets.
 */
function expand(config: LintPreset): LintRules {
  let rules: LintRules = {};
  for (const preset of config.extends ?? []) {
    rules = mergeRules(rules, expand(preset));
  }
  return mergeRules(rules, config.rules ?? {});
}

/**
 * Merges rule overrides while retaining inherited options for severity-only settings.
 * @param base - Rules using validated catalog names and option fields.
 * @param overrides - Validated fields. Undefined values preserve prior values, and arrays replace them.
 * @returns A new rule map without mutating either input. Retained or replacement arrays can share input references.
 */
function mergeRules(base: LintRules, overrides: LintRules): LintRules {
  const rules = { ...base };
  for (const [name, setting] of Object.entries(overrides)) {
    if (setting === undefined) {
      continue;
    }
    const key = name as LintRule;
    const previous = base[key];
    const merged =
      lintRules[key].defaults === null
        ? setting
        : {
            ...(typeof previous === "string" ? { severity: previous } : previous),
            ...(typeof setting === "string"
              ? { severity: setting }
              : Object.fromEntries(
                  Object.entries(setting).filter(([, value]) => value !== undefined),
                )),
          };
    Object.assign(rules, { [key]: merged });
  }
  return rules;
}

/**
 * Checks effective rule settings after all presets, overrides, and defaults have been applied.
 * @param value - Complete rules whose required options can now be checked.
 * @param location - Field path for the rules object in validation errors.
 * @throws If rule settings are invalid or an active theme restriction has no token patterns.
 */
function validateResolvedRules(value: ResolvedLintRules, location: string) {
  validate(lintRulesSchema, value, location);
  validate(
    v.pipe(
      ruleOptions["no-restricted-theme-tokens"],
      v.forward(
        v.check(
          (setting) => setting.severity === "off" || Boolean(setting.tokens?.length),
          "must contain custom-property patterns when the rule is active",
        ),
        ["tokens"],
      ),
    ),
    value["no-restricted-theme-tokens"],
    `${location}.no-restricted-theme-tokens`,
  );
}
