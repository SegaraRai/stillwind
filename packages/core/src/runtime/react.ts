import { createCva, type CompiledCvaConfig, type CvaValue } from "./cva.ts";
import { mergeFragments, serializeStyle, toFragment } from "./fragment.ts";

export * from "./shared.ts";

export const stillwindClassKey = /* @__PURE__ */ Symbol("stillwind.react.class");

export const stillwindStyleKey = /* @__PURE__ */ Symbol("stillwind.react.style");

export const stillwindCustomPropertiesKey = /* @__PURE__ */ Symbol(
  "stillwind.react.custom-properties",
);

export const options = [
  "className",
  "object",
  stillwindClassKey,
  stillwindStyleKey,
  stillwindCustomPropertiesKey,
] as const;

/** Materializes ordered fragments with the react class and style conventions. */
export function stillwindMerge(...values: readonly unknown[]): Record<string, unknown> {
  return mergeFragments(options, values);
}

/** Converts a composition base to a fragment with only its react class. */
export function stillwindToFragment(
  value: Record<PropertyKey, unknown>,
): Record<PropertyKey, unknown> {
  return toFragment(options, value);
}

/** Returns the style representation accepted by react; null and undefined omit style. */
export function stillwindStyleValue(value: unknown): unknown {
  return value == null ? undefined : serializeStyle(value, "object");
}

/** Selects compiled CVA values and materializes fresh react props. */
export function stillwindCva(base?: CvaValue, config?: CompiledCvaConfig) {
  return createCva(options, base, config);
}
