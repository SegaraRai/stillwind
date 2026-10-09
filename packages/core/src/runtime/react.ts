import { mergeFragments, serializeStyle, toFragment } from "./fragment.ts";

export * from "./shared.ts";

export const stillwindClassKey = /* @__PURE__ */ Symbol("stillwind.react.class");

export const stillwindStyleKey = /* @__PURE__ */ Symbol("stillwind.react.style");

export const stillwindCustomPropertiesKey = /* @__PURE__ */ Symbol(
  "stillwind.react.custom-properties",
);

const options = [
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

/** Converts an object containing react props to fragment channels. */
export function stillwindToFragment(
  value: Record<PropertyKey, unknown>,
): Record<PropertyKey, unknown> {
  return toFragment(options, value);
}

/** Returns the style representation accepted by react; null and undefined omit style. */
export function stillwindStyleValue(value: unknown): unknown {
  return value == null ? undefined : serializeStyle(value, "object");
}
