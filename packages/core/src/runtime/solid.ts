import { createCva, type CompiledCvaConfig, type CvaValue } from "./cva.ts";
import { mergeFragments, serializeStyle, toFragment } from "./fragment.ts";

export * from "./shared.ts";

export const stillwindClassKey = /* @__PURE__ */ Symbol("stillwind.solid.class");

export const stillwindStyleKey = /* @__PURE__ */ Symbol("stillwind.solid.style");

export const stillwindCustomPropertiesKey = /* @__PURE__ */ Symbol(
  "stillwind.solid.custom-properties",
);

export const options = [
  "class",
  "object",
  stillwindClassKey,
  stillwindStyleKey,
  stillwindCustomPropertiesKey,
] as const;

/** Materializes ordered fragments with the solid class and style conventions. */
export function stillwindMerge(...values: readonly unknown[]): Record<string, unknown> {
  return mergeFragments(options, values);
}

/** Converts a composition base to a fragment with only its solid class. */
export function stillwindToFragment(
  value: Record<PropertyKey, unknown>,
): Record<PropertyKey, unknown> {
  return toFragment(options, value);
}

/** Returns the style representation accepted by solid; null and undefined omit style. */
export function stillwindStyleValue(value: unknown): unknown {
  return value == null ? undefined : serializeStyle(value, "object");
}

/** Selects compiled CVA values and materializes fresh solid props. */
export function stillwindCva(base?: CvaValue, config?: CompiledCvaConfig) {
  return createCva(options, base, config);
}
