/**
 * Fragment keys and production operations for each output format.
 *
 * Each format owns distinct symbol keys, so a fragment must be materialized by the format that
 * created it. Export names start with the format name because all formats share one entrypoint.
 */

import { createCva, type CompiledCvaConfig, type CvaValue } from "./cva.ts";
import {
  mergeFragments,
  serializeStyle,
  toFragment,
  type FragmentRuntimeOptions,
} from "./fragment.ts";

export const stillwindClassObjectClassKey = /* @__PURE__ */ Symbol("stillwind.class-object.class");

export const stillwindClassObjectStyleKey = /* @__PURE__ */ Symbol("stillwind.class-object.style");

export const stillwindClassObjectCustomPropertiesKey = /* @__PURE__ */ Symbol(
  "stillwind.class-object.custom-properties",
);

export const classObjectOptions: FragmentRuntimeOptions = [
  "class",
  "object",
  stillwindClassObjectClassKey,
  stillwindClassObjectStyleKey,
  stillwindClassObjectCustomPropertiesKey,
];

/** Materializes ordered fragments with `class` and object styles. */
export function stillwindClassObjectMerge(...values: readonly unknown[]): Record<string, unknown> {
  return mergeFragments(classObjectOptions, values);
}

/** Converts either class prop of a composition base to a class-only fragment. */
export function stillwindClassObjectToFragment(
  value: Record<PropertyKey, unknown>,
): Record<PropertyKey, unknown> {
  return toFragment(classObjectOptions, value);
}

/** Returns the supplied style payload, or undefined for null or undefined. */
export function stillwindClassObjectStyleValue(value: unknown): unknown {
  return value == null ? undefined : serializeStyle(value, "object");
}

/** Selects compiled CVA values and returns fresh props with `class` and object styles. */
export function stillwindClassObjectCva(base?: CvaValue, config?: CompiledCvaConfig) {
  return createCva(classObjectOptions, base, config);
}

export const stillwindClassStringClassKey = /* @__PURE__ */ Symbol("stillwind.class-string.class");

export const stillwindClassStringStyleKey = /* @__PURE__ */ Symbol("stillwind.class-string.style");

export const stillwindClassStringCustomPropertiesKey = /* @__PURE__ */ Symbol(
  "stillwind.class-string.custom-properties",
);

export const classStringOptions: FragmentRuntimeOptions = [
  "class",
  "string",
  stillwindClassStringClassKey,
  stillwindClassStringStyleKey,
  stillwindClassStringCustomPropertiesKey,
];

/** Materializes ordered fragments with `class` and string styles. */
export function stillwindClassStringMerge(...values: readonly unknown[]): Record<string, unknown> {
  return mergeFragments(classStringOptions, values);
}

/** Converts either class prop of a composition base to a class-only fragment. */
export function stillwindClassStringToFragment(
  value: Record<PropertyKey, unknown>,
): Record<PropertyKey, unknown> {
  return toFragment(classStringOptions, value);
}

/** Serializes object style declarations or preserves a string. Other inputs omit style. */
export function stillwindClassStringStyleValue(value: unknown): unknown {
  return value == null ? undefined : serializeStyle(value, "string");
}

/** Selects compiled CVA values and returns fresh props with `class` and string styles. */
export function stillwindClassStringCva(base?: CvaValue, config?: CompiledCvaConfig) {
  return createCva(classStringOptions, base, config);
}

export const stillwindClassNameObjectClassKey = /* @__PURE__ */ Symbol(
  "stillwind.class-name-object.class",
);

export const stillwindClassNameObjectStyleKey = /* @__PURE__ */ Symbol(
  "stillwind.class-name-object.style",
);

export const stillwindClassNameObjectCustomPropertiesKey = /* @__PURE__ */ Symbol(
  "stillwind.class-name-object.custom-properties",
);

export const classNameObjectOptions: FragmentRuntimeOptions = [
  "className",
  "object",
  stillwindClassNameObjectClassKey,
  stillwindClassNameObjectStyleKey,
  stillwindClassNameObjectCustomPropertiesKey,
];

/** Materializes ordered fragments with `className` and object styles. */
export function stillwindClassNameObjectMerge(
  ...values: readonly unknown[]
): Record<string, unknown> {
  return mergeFragments(classNameObjectOptions, values);
}

/** Converts either class prop of a composition base to a class-only fragment. */
export function stillwindClassNameObjectToFragment(
  value: Record<PropertyKey, unknown>,
): Record<PropertyKey, unknown> {
  return toFragment(classNameObjectOptions, value);
}

/** Returns the supplied style payload, or undefined for null or undefined. */
export function stillwindClassNameObjectStyleValue(value: unknown): unknown {
  return value == null ? undefined : serializeStyle(value, "object");
}

/** Selects compiled CVA values and returns fresh props with `className` and object styles. */
export function stillwindClassNameObjectCva(base?: CvaValue, config?: CompiledCvaConfig) {
  return createCva(classNameObjectOptions, base, config);
}
