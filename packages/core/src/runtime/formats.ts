/**
 * Fragment keys and production operations for each output format.
 *
 * Each format owns distinct symbol keys, so a fragment must be materialized by the format that
 * created it. Export names start with the format name because all formats share one entrypoint.
 */

import {
  forwardFragment,
  mergeFragments,
  serializeStyle,
  toFragment,
  type FragmentRuntimeOptions,
} from "./fragment.ts";
import { createRecipe, type CompiledRecipeConfig, type RecipeValue } from "./recipe.ts";

export const stillwindClassObjectClassKey = /* @__PURE__ */ Symbol("stillwind.class-object.class");

export const stillwindClassObjectStyleKey = /* @__PURE__ */ Symbol("stillwind.class-object.style");

export const stillwindClassObjectCustomPropertiesKey = /* @__PURE__ */ Symbol(
  "stillwind.class-object.custom-properties",
);

const stillwindClassObjectForwardKey = /* @__PURE__ */ Symbol("stillwind.class-object.forward");

export const classObjectOptions: FragmentRuntimeOptions = [
  "class",
  "object",
  stillwindClassObjectClassKey,
  stillwindClassObjectStyleKey,
  stillwindClassObjectCustomPropertiesKey,
  stillwindClassObjectForwardKey,
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

/** Creates a fragment from forwarded props for a framework without style arrays. */
export function stillwindClassObjectForward(value: unknown): Record<PropertyKey, unknown> {
  return forwardFragment(classObjectOptions, value, false);
}

/** Creates a fragment from forwarded props for a framework that accepts style arrays. */
export function stillwindClassObjectForwardArrays(value: unknown): Record<PropertyKey, unknown> {
  return forwardFragment(classObjectOptions, value, true);
}

/** Selects compiled recipe values and returns fresh props with `class` and object styles. */
export function stillwindClassObjectRecipe(base?: RecipeValue, config?: CompiledRecipeConfig) {
  return createRecipe(classObjectOptions, base, config);
}

export const stillwindClassStringClassKey = /* @__PURE__ */ Symbol("stillwind.class-string.class");

export const stillwindClassStringStyleKey = /* @__PURE__ */ Symbol("stillwind.class-string.style");

export const stillwindClassStringCustomPropertiesKey = /* @__PURE__ */ Symbol(
  "stillwind.class-string.custom-properties",
);

const stillwindClassStringForwardKey = /* @__PURE__ */ Symbol("stillwind.class-string.forward");

export const classStringOptions: FragmentRuntimeOptions = [
  "class",
  "string",
  stillwindClassStringClassKey,
  stillwindClassStringStyleKey,
  stillwindClassStringCustomPropertiesKey,
  stillwindClassStringForwardKey,
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

/** Creates a fragment from forwarded props for a framework without style arrays. */
export function stillwindClassStringForward(value: unknown): Record<PropertyKey, unknown> {
  return forwardFragment(classStringOptions, value, false);
}

/** Selects compiled recipe values and returns fresh props with `class` and string styles. */
export function stillwindClassStringRecipe(base?: RecipeValue, config?: CompiledRecipeConfig) {
  return createRecipe(classStringOptions, base, config);
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

const stillwindClassNameObjectForwardKey = /* @__PURE__ */ Symbol(
  "stillwind.class-name-object.forward",
);

export const classNameObjectOptions: FragmentRuntimeOptions = [
  "className",
  "object",
  stillwindClassNameObjectClassKey,
  stillwindClassNameObjectStyleKey,
  stillwindClassNameObjectCustomPropertiesKey,
  stillwindClassNameObjectForwardKey,
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

/** Creates a fragment from forwarded props for a framework without style arrays. */
export function stillwindClassNameObjectForward(value: unknown): Record<PropertyKey, unknown> {
  return forwardFragment(classNameObjectOptions, value, false);
}

/** Selects compiled recipe values and returns fresh props with `className` and object styles. */
export function stillwindClassNameObjectRecipe(base?: RecipeValue, config?: CompiledRecipeConfig) {
  return createRecipe(classNameObjectOptions, base, config);
}
