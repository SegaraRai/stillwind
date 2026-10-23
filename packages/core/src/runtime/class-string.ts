import { createCva, type CompiledCvaConfig, type CvaValue } from "./cva.ts";
import { mergeFragments, serializeStyle, toFragment } from "./fragment.ts";

export * from "./shared.ts";

export const stillwindClassKey = /* @__PURE__ */ Symbol("stillwind.class-string.class");

export const stillwindStyleKey = /* @__PURE__ */ Symbol("stillwind.class-string.style");

export const stillwindCustomPropertiesKey = /* @__PURE__ */ Symbol(
  "stillwind.class-string.custom-properties",
);

export const options = [
  "class",
  "string",
  stillwindClassKey,
  stillwindStyleKey,
  stillwindCustomPropertiesKey,
] as const;

/** Materializes ordered fragments with `class` and string styles. */
export function stillwindMerge(...values: readonly unknown[]): Record<string, unknown> {
  return mergeFragments(options, values);
}

/** Converts a composition base to a fragment containing only its `class`. */
export function stillwindToFragment(
  value: Record<PropertyKey, unknown>,
): Record<PropertyKey, unknown> {
  return toFragment(options, value);
}

/** Serializes object style declarations or preserves a string. Other inputs omit style. */
export function stillwindStyleValue(value: unknown): unknown {
  return value == null ? undefined : serializeStyle(value, "string");
}

/** Selects compiled CVA values and returns fresh props with `class` and string styles. */
export function stillwindCva(base?: CvaValue, config?: CompiledCvaConfig) {
  return createCva(options, base, config);
}
