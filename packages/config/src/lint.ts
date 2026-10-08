/** Ordered lint preset composition and validation for all configuration hosts. */

import * as v from "valibot";

import { lintPresets } from "./lint-presets.ts";
import { lengthUnits, lintRules, type LintRule, type SimpleLintRule } from "./lint-rules.ts";
import { nonemptyStringList, nonemptyText, object, strictObject, validate } from "./validation.ts";

/** Reporting policy for a lint rule. Off disables its diagnostics and automatic fixes. */
export type LintSeverity = "warn" | "deny" | "off";

/** Detection and reporting policies for source utilities and compiler ordering. */
export type LintRules = Partial<Record<SimpleLintRule, LintSeverity>> & {
  "no-arbitrary-spacing"?: LintSeverity | ArbitrarySpacingOptions;
  "no-restricted-utilities"?: LintSeverity | RestrictedUtilitiesOptions;
  "no-restricted-declarations"?: LintSeverity | RestrictedDeclarationsOptions;
  "conflicting-utilities"?: LintSeverity | ConflictingUtilitiesOptions;
  "length-units"?: LintSeverity | LengthUnitsOptions;
  "no-restricted-theme-tokens"?: LintSeverity | RestrictedThemeTokensOptions;
  "canonical-utilities"?: LintSeverity | CanonicalUtilitiesOptions;
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

/** Project lint configuration. Omitted extends uses no presets when this object is present. */
export type LintConfig = LintPreset;

/** Complete reporting settings with default detection options applied. */
export type ResolvedLintRules = {
  [K in keyof LintRules]-?: Exclude<LintRules[K], LintSeverity | undefined> extends never
    ? LintSeverity
    : Required<Exclude<LintRules[K], LintSeverity | undefined>> & { severity: LintSeverity };
};

/** Selects additional domains beyond margin, padding, and gap. */
export type ArbitrarySpacingOptions = {
  /** Omission preserves inherited severity, or uses warn for a newly configured rule. */
  severity?: LintSeverity;
  /** Case-insensitive full property globs using `*` and `?`. */
  properties?: readonly string[];
};

/** Forbids utility spellings independently of CSS resolution. */
export type RestrictedUtilitiesOptions = {
  /** Omission preserves inherited severity, or uses warn for a newly configured rule. */
  severity?: LintSeverity;
  /** Full utility globs exclude enclosing variants and important modifiers. */
  utilities?: readonly {
    pattern: string;
    message?: string;
  }[];
};

/** Forbids resolved declarations, including parsed shorthand components. */
export type RestrictedDeclarationsOptions = {
  /** Omission preserves inherited severity, or uses warn for a newly configured rule. */
  severity?: LintSeverity;
  /** Property and value globs ignore ASCII case. Values match complete serialized CSS text. */
  rules?: readonly {
    properties: readonly string[];
    values: readonly string[];
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

/** Controls nonzero CSS lengths; later matching rules replace earlier restrictions. */
export type LengthUnitsOptions = {
  /** Omission preserves inherited severity, or uses warn for a newly configured rule. */
  severity?: LintSeverity;
  /** Property names use case-insensitive full-name globs with `*` and `?`. */
  rules?: readonly {
    properties: readonly string[];
    disallow: readonly string[];
  }[];
};

/** Selects theme token names without restricting semantic aliases of those tokens. */
export type RestrictedThemeTokensOptions = {
  /** Omission preserves inherited severity, or uses warn for a newly configured rule. */
  severity?: LintSeverity;
  /** Case-sensitive full custom-property name globs with `*` and `?`. */
  tokens?: readonly string[];
};

/** Controls replacement of equivalent utility spellings. */
export type CanonicalUtilitiesOptions = {
  /** Omission preserves inherited severity, or uses warn for a newly configured rule. */
  severity?: LintSeverity;
  /** Converts pixel spacing to rem when the active spacing scale uses rem. Defaults to true. */
  pxToRem?: boolean;
  /** Positive finite number of pixels per rem. Defaults to 16. Used only for spacing conversion. */
  rootFontSize?: number;
};

/**
 * Applies an inline override to project lint settings without expanding presets.
 * Omitted extends preserves the project's list; a supplied list replaces it.
 * Omitted project settings select recommended. Rule fields merge and arrays replace.
 * Neither input is modified.
 * @param base - Project settings, or undefined to use recommended.
 * @param override - Inline settings; undefined preserves the project settings.
 * @returns A configuration with inherited rule options preserved.
 */
export function mergeLintConfig(
  base: LintConfig | undefined,
  override: LintConfig | undefined,
): LintConfig {
  const previous: LintConfig = base ?? { extends: [lintPresets.recommended] };
  if (override === undefined) {
    return previous;
  }

  return {
    ...previous,
    ...(override.name === undefined ? {} : { name: override.name }),
    ...(override.extends === undefined ? {} : { extends: override.extends }),
    rules: mergeRules(previous.rules ?? {}, override.rules ?? {}),
  };
}

/**
 * Expands presets in order and applies rule defaults without modifying the input.
 * @param config - Project lint settings. Undefined selects recommended; an empty object disables all rules.
 * @param location - Field path used in configuration errors.
 * @returns Every rule's severity and detection options, wrapped in rules for repeatable normalization.
 * @throws If a preset has a cycle, an unknown field, or invalid effective rule options.
 */
export function resolveLintConfig(
  config?: LintConfig,
  location = "config.lint",
): { rules: ResolvedLintRules } {
  const value = config ?? { extends: [lintPresets.recommended] };
  validatePreset(value, location, new Set());
  const rules = resolvedRules(value);
  validateResolvedRules(rules, `${location}.rules`);
  return { rules };
}

/** Validates preset fields and cycles; partial overrides defer required options until composition. */
export function validateLintConfig(value: unknown, location: string, partial: boolean): void {
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
  "length-units": strictObject({
    severity: v.optional(severitySchema),
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
  "no-restricted-theme-tokens": strictObject({
    severity: v.optional(severitySchema),
    tokens: v.optional(v.array(tokenGlob)),
  }),
  "canonical-utilities": strictObject({
    severity: v.optional(severitySchema),
    pxToRem: v.optional(v.boolean()),
    rootFontSize: v.optional(
      v.pipe(
        v.number(),
        v.finite(),
        v.check((value) => value > 0, "must be positive and finite"),
      ),
    ),
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

function resolvedRules(config: LintConfig): ResolvedLintRules {
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

function expand(config: LintConfig): LintRules {
  let rules: LintRules = {};
  for (const preset of config.extends ?? []) {
    rules = mergeRules(rules, expand(preset));
  }
  return mergeRules(rules, config.rules ?? {});
}

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
