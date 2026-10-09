import { mergeFragments, serializeStyle, toFragment } from "./fragment.ts";

export * from "./shared.ts";

export const stillwindClassKey = /* @__PURE__ */ Symbol("stillwind.solid.class");
export const stillwindStyleKey = /* @__PURE__ */ Symbol("stillwind.solid.style");
export const stillwindCustomPropertiesKey = /* @__PURE__ */ Symbol(
  "stillwind.solid.custom-properties",
);

const options = {
  classProperty: "class",
  styleFormat: "object",
  stillwindClassKey,
  stillwindStyleKey,
  stillwindCustomPropertiesKey,
} as const;

/** Materializes ordered fragments with the solid class and style conventions. */
export function stillwindMerge(...values: readonly unknown[]): Record<string, unknown> {
  return mergeFragments(options, values);
}

/** Converts an object containing solid props to fragment channels. */
export function stillwindToFragment(
  value: Record<PropertyKey, unknown>,
): Record<PropertyKey, unknown> {
  return toFragment(options, value);
}

/** Returns the style representation accepted by solid; null and undefined omit style. */
export function stillwindStyleValue(value: unknown): unknown {
  return value == null ? undefined : serializeStyle(value, "object");
}
