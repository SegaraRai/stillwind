import { decodeArbitraryValue } from "@stillwind/native";
import type { PluginAPI } from "tailwindcss/plugin";

import { serializeCss, splitTopLevel, utilityBodies, withAlpha } from "./css.ts";
import { inferDataType, type DataType } from "./data-type.ts";

/** Options of one `matchUtilities()` call. */
type MatchOptions = NonNullable<Parameters<PluginAPI["matchUtilities"]>[1]>;

/** Callback of one utility root in `matchUtilities()`. */
type UtilityCallback = Parameters<PluginAPI["matchUtilities"]>[0][string];

/** Callback of `matchVariant()`, which returns a selector, an at-rule, or a list of them. */
export type VariantCallback = (
  value: unknown,
  extra: {
    modifier: string | null;
  },
) => string | string[];

/** Options of one `matchVariant()` call. */
export interface VariantOptions {
  /** Named values, with `DEFAULT` for the bare root. */
  values?: Record<string, unknown>;
  /** Orders the generated variants; without it, the order of `values` decides. */
  sort?: (
    a: {
      value: unknown;
      modifier: string | null;
    },
    b: {
      value: unknown;
      modifier: string | null;
    },
  ) => number;
}

/** One root of a `matchUtilities()` call and its results so far. */
export interface DynamicUtility {
  /** Index of the `@plugin` statement whose handler registered the utility. */
  owner: number;
  /** Utility root, such as `scrollbar`. */
  name: string;
  callback: UtilityCallback;
  options: MatchOptions;
  /** Rules that carry the selectors of the callback's results for `options.values`, for marker classes. */
  markerSource: string;
  /** Generated bodies by request, null when the callback stops evaluation. */
  results: Map<string, string[] | null | undefined>;
}

/** One variant that a `matchVariant()` callback generated. */
interface VariantResult {
  /** Complete variant name, such as `state-[open]`. */
  name: string;
  /** Value passed to the callback. */
  value: unknown;
  /** Modifier passed to the callback. */
  modifier: string | null;
  /** Variant body with `@slot`. */
  body: string;
}

/** One `matchVariant()` registration and its results so far. */
export interface DynamicVariant {
  /** Index of the `@plugin` statement whose handler registered the variant. */
  owner: number;
  /** Variant root, such as `state`. */
  name: string;
  callback: VariantCallback;
  options: VariantOptions;
  /** CSS that the callback generates for the values in `options.values`, for marker classes. */
  markerSource: string;
  /** Generated variants by complete name. */
  results: Map<string, VariantResult>;
}

/**
 * Runs a `matchUtilities()` or `matchVariant()` callback for each named value and joins the rules
 * that carry its selectors.
 *
 * The compiler collects marker classes from the selectors of callback results that candidates
 * request. The host passes these rules in `@stillwind-marker-source`, so these classes are
 * markers before any candidate requests a value, and the class that a candidate adds does not
 * depend on which module the host compiles first. A value whose callback throws contributes
 * nothing.
 *
 * @param values - Named values of the registration. Keys that start with `__` are skipped, as
 * Tailwind skips them when it lists candidates.
 * @param run - Returns the CSS rules that carry the selectors of the callback's result for one
 * value.
 * @returns The distinct rules, or an empty string when no value has a selector.
 */
export function probeCss(
  values: Record<string, unknown> | undefined,
  run: (value: unknown) => readonly string[],
): string {
  const rules = new Set<string>();
  for (const [key, value] of Object.entries(values ?? {})) {
    if (key.startsWith("__")) {
      continue;
    }
    try {
      for (const rule of run(value)) {
        rules.add(rule);
      }
    } catch {}
  }
  return [...rules].join("");
}

/**
 * Whether a candidate or variant belongs to a registered root.
 *
 * @param candidate - Name without a leading `-`.
 * @param root - Registered root, such as `scrollbar`.
 * @returns Whether the name is the root or continues it with `-` or `/`.
 */
export function member(candidate: string, root: string): boolean {
  return candidate === root || candidate.startsWith(`${root}-`) || candidate.startsWith(`${root}/`);
}

/**
 * Converts a variant definition that a plugin returns into a `@custom-variant` body.
 *
 * @param value - Selector or at-rule string, CSS object, or a list of them.
 * @returns CSS with `@slot`. A string without `@slot` gets one at its innermost block, or a
 * block of its own.
 */
export function variantBody(value: unknown): string {
  if (Array.isArray(value)) {
    return value.map(variantBody).join("\n");
  }
  if (typeof value !== "string") {
    return serializeCss(value);
  }
  if (value.includes("@slot")) {
    return value;
  }
  if (value.includes("{")) {
    return value.replace("}", "{@slot;}}");
  }
  return `${value}{@slot;}`;
}

/** A candidate value or modifier, as Tailwind's candidate parser reads it. */
type CandidatePart =
  | {
      kind: "named";
      /** Text as written. */
      value: string;
    }
  | {
      kind: "arbitrary";
      /** Decoded value; a `(--name)` shorthand becomes `var(--name)`. */
      value: string;
      /** Explicit type hint such as `length` in `[length:1px]`, or null without one. */
      dataType: string | null;
    };

/**
 * Whether a decoded arbitrary value has no top-level `;` or `}`, which Tailwind rejects.
 *
 * @param value - Decoded value.
 * @returns Whether the value can be used.
 */
function validArbitrary(value: string): boolean {
  return splitTopLevel(value, ";").length === 1 && splitTopLevel(value, "}").length === 1;
}

/**
 * Parses the value part of a candidate as Tailwind's candidate parser does.
 *
 * @param text - Text after the root and its `-`, such as `4`, `[length:1px]`, or
 * `(length:--size)`.
 * @returns The value, or undefined when Tailwind rejects the candidate.
 */
function parseValue(text: string): CandidatePart | undefined {
  let inner: string;
  if (text.startsWith("(") && text.endsWith(")")) {
    // Tailwind rewrites `(type:--name)` to `[type:var(--name)]` before decoding.
    const parts = splitTopLevel(text.slice(1, -1), ":");
    const [dataType, variable] = parts.length === 2 ? parts : [null, parts.join(":")];
    if (!variable.startsWith("--") || !validArbitrary(variable)) {
      return undefined;
    }
    inner = dataType === null ? `var(${variable})` : `${dataType}:var(${variable})`;
  } else if (text.startsWith("[") && text.endsWith("]")) {
    inner = text.slice(1, -1);
  } else {
    return {
      kind: "named",
      value: text,
    };
  }
  let value = decodeArbitraryValue(inner);
  if (!validArbitrary(value)) {
    return undefined;
  }
  // A type hint consists of lowercase letters and hyphens before the first colon.
  let dataType: string | null = null;
  const hint = /^([a-z-]*):/.exec(value);
  if (hint) {
    if (!hint[1]) {
      return undefined;
    }
    dataType = hint[1];
    value = value.slice(hint[0].length);
  }
  return value.trim()
    ? {
        kind: "arbitrary",
        value,
        dataType,
      }
    : undefined;
}

/**
 * Parses a `/` modifier as Tailwind's candidate parser does.
 *
 * @param text - Modifier text without the slash.
 * @returns The modifier, or undefined when Tailwind rejects the candidate.
 */
function parseModifier(text: string): CandidatePart | undefined {
  if (text.startsWith("[") && text.endsWith("]")) {
    const value = decodeArbitraryValue(text.slice(1, -1));
    return validArbitrary(value) && value.trim()
      ? {
          kind: "arbitrary",
          value,
          dataType: null,
        }
      : undefined;
  }
  if (text.startsWith("(") && text.endsWith(")")) {
    const variable = text.slice(1, -1);
    return variable.startsWith("--") && validArbitrary(variable)
      ? {
          kind: "arbitrary",
          value: decodeArbitraryValue(`var(${variable})`),
          dataType: null,
        }
      : undefined;
  }
  return text
    ? {
        kind: "named",
        value: text,
      }
    : undefined;
}

/**
 * Runs a `matchUtilities` callback for one candidate as Tailwind's compatibility layer does.
 *
 * @param candidate - Utility key, such as `-probe-[3px]/mirror`.
 * @param registration - Registration whose root, callback, and options apply.
 * @returns The bodies of the callback's targets, undefined when the registration does not apply,
 * and null when an arbitrary value has an invalid modifier, which ends Tailwind's evaluation of
 * later definitions.
 */
export function resolveUtility(
  candidate: string,
  registration: DynamicUtility,
): string[] | null | undefined {
  const { name, callback, options } = registration;
  const negative = candidate.startsWith("-");
  const raw = negative ? candidate.slice(1) : candidate;
  if (!member(raw, name) || (negative && !options.supportsNegativeValues)) {
    return;
  }
  const [stem, ...modifierParts] = splitTopLevel(raw, "/");
  if (modifierParts.length > 1) {
    return;
  }
  const modifierText = modifierParts[0];
  const valueText = stem === name ? undefined : stem.slice(name.length + 1);
  const parsed = valueText === undefined ? null : parseValue(valueText);
  const candidateModifier = modifierText === undefined ? null : parseModifier(modifierText);
  if (parsed === undefined || candidateModifier === undefined) {
    return;
  }
  const types = Array.isArray(options.type) ? options.type : [options.type ?? "any"];
  if (parsed?.kind === "arbitrary" && types.length > 0 && !types.includes("any")) {
    if (
      parsed.dataType
        ? !types.includes(parsed.dataType)
        : !inferDataType(parsed.value, types as DataType[])
    ) {
      return;
    }
  }
  const isColor = types.includes("color");
  // Color utilities accept these keywords unless their values define the keys, as in Tailwind.
  const values: Record<string, unknown> = isColor
    ? {
        inherit: "inherit",
        transparent: "transparent",
        current: "currentcolor",
        ...options.values,
      }
    : (options.values ?? {});
  // A named value with a named modifier, such as `w-1/2`, may be a fraction key.
  const fraction =
    parsed?.kind === "named" && candidateModifier?.kind === "named"
      ? `${parsed.value}/${candidateModifier.value}`
      : null;
  let value: unknown = null;
  let ignoreModifier = false;
  if (!parsed) {
    value = values.DEFAULT ?? null;
  } else if (parsed.kind === "arbitrary") {
    value = parsed.value;
  } else if (fraction && Object.hasOwn(values, fraction)) {
    value = values[fraction];
    ignoreModifier = true;
  } else if (Object.hasOwn(values, parsed.value)) {
    value = values[parsed.value];
  } else if (typeof values.__BARE_VALUE__ === "function") {
    value =
      values.__BARE_VALUE__({
        kind: "named",
        value: parsed.value,
        fraction,
      }) ?? null;
    ignoreModifier = fraction !== null && typeof value === "string" && value.includes("/");
  }
  if (value === null || value === undefined) {
    return;
  }
  let modifier: string | null = null;
  if (candidateModifier) {
    const modifiers = options.modifiers;
    if (modifiers === "any" || candidateModifier.kind === "arbitrary") {
      modifier = candidateModifier.value;
    } else if (modifiers && Object.hasOwn(modifiers, candidateModifier.value)) {
      modifier = modifiers[candidateModifier.value];
    } else if (isColor && !Number.isNaN(Number(candidateModifier.value))) {
      modifier = `${candidateModifier.value}%`;
    }
    if (modifier === null && !ignoreModifier) {
      return parsed?.kind === "arbitrary" ? null : undefined;
    }
  }
  // Like Tailwind, these interpolate the value as text; color and negative registrations hold
  // strings or numbers.
  if (isColor && modifier !== null) {
    value = withAlpha(String(value as string), modifier);
  }
  if (negative) {
    value = `calc(${String(value as string)} * -1)`;
  }
  return utilityBodies(
    // Tailwind passes a value from `values` as it is, so a number stays a number.
    (
      callback as (
        value: unknown,
        extra: {
          modifier: string | null;
        },
      ) => unknown
    )(value, {
      modifier,
    }),
  );
}

/**
 * Selects the callback arguments of a `matchVariant()` registration for one variant name, as
 * Tailwind's compatibility layer does.
 *
 * The bare root uses the `DEFAULT` value. A named value must be a string in `values`, so
 * `values: { one: 1 }` defines no `name-one` variant. An arbitrary value passes its decoded text.
 *
 * @param candidate - Complete variant name, such as `state-[open]/peer`.
 * @param registration - Registration whose root and values apply.
 * @returns The variant name, value, and modifier, or undefined when the registration has no
 * such value.
 */
export function resolveVariant(
  candidate: string,
  registration: DynamicVariant,
):
  | {
      name: string;
      value: unknown;
      modifier: string | null;
    }
  | undefined {
  const { name, options } = registration;
  if (!member(candidate, name)) {
    return;
  }
  const [stem, ...modifierParts] = splitTopLevel(candidate, "/");
  if (modifierParts.length > 1) {
    return;
  }
  const valueText = stem === name ? undefined : stem.slice(name.length + 1);
  const parsed = valueText === undefined ? null : parseValue(valueText);
  const modifier = modifierParts[0] === undefined ? null : parseModifier(modifierParts[0]);
  if (parsed === undefined || modifier === undefined) {
    return;
  }
  const values = options.values;
  let value: unknown;
  if (!parsed) {
    if (!values || !("DEFAULT" in values)) {
      return;
    }
    value = values.DEFAULT;
  } else if (parsed.kind === "arbitrary") {
    value = parsed.value;
  } else {
    value = values && Object.hasOwn(values, parsed.value) ? values[parsed.value] : undefined;
    if (typeof value !== "string") {
      return;
    }
  }
  return {
    name: candidate,
    value,
    modifier: modifier?.value ?? null,
  };
}
