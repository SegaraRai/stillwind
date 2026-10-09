import { mergeFragments, serializeStyle, toFragment } from "./fragment.ts";

export * from "./shared.ts";

export const stillwindClassKey = /* @__PURE__ */ Symbol("stillwind.preact.class");

export const stillwindStyleKey = /* @__PURE__ */ Symbol("stillwind.preact.style");

export const stillwindCustomPropertiesKey = /* @__PURE__ */ Symbol(
  "stillwind.preact.custom-properties",
);

const options = [
  "class",
  "object",
  stillwindClassKey,
  stillwindStyleKey,
  stillwindCustomPropertiesKey,
] as const;

/** Materializes ordered fragments with the preact class and style conventions. */
export function stillwindMerge(...values: readonly unknown[]): Record<string, unknown> {
  return mergeFragments(options, values);
}

/** Converts an object containing preact props to fragment channels. */
export function stillwindToFragment(
  value: Record<PropertyKey, unknown>,
): Record<PropertyKey, unknown> {
  return toFragment(options, value);
}

/** Returns the style representation accepted by preact; null and undefined omit style. */
export function stillwindStyleValue(value: unknown): unknown {
  return value == null ? undefined : serializeStyle(value, "object");
}
