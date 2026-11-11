/** Runtime validation selected explicitly by development compilations. */

import { createCva, type CompiledCvaConfig, type CvaValue } from "./cva.ts";
import { classNameObjectOptions, classObjectOptions, classStringOptions } from "./formats.ts";
import { isObject, mergeFragments, toFragment, type FragmentRuntimeOptions } from "./fragment.ts";
import { stillwindClsx, stillwindViewTransitionName } from "./shared.ts";

/**
 * Reads a compiled ref's class token once.
 *
 * @param value - A ref record whose `.c` property contains a nonempty string.
 * @returns The class token.
 * @throws TypeError if the record or token is missing, or the token is empty or not a string.
 */
export function stillwindRefClassChecked(value: unknown): string {
  const token = isObject(value) ? value.c : undefined;
  if (typeof token !== "string" || !token) {
    throw new TypeError("Stillwind received a ref without a class token in .c.");
  }
  return token;
}

/**
 * Validates external class inputs before concatenating them.
 *
 * @param values - Strings, numbers, bigints, nested arrays, or objects whose truthy entries enable their keys.
 * Booleans, null, and undefined add no text.
 * @returns Class text in contribution order without deduplication.
 * @throws TypeError when an input or nested array contains a function or symbol.
 */
export function stillwindClsxChecked(...values: readonly unknown[]): string {
  function check(value: unknown): void {
    if (value == null || ["string", "number", "bigint", "boolean"].includes(typeof value)) {
      return;
    }
    if (Array.isArray(value)) {
      value.forEach(check);
      return;
    }
    if (isObject(value)) {
      return;
    }
    throw new TypeError(`Stillwind received an unsupported class value: ${typeof value}.`);
  }
  values.forEach(check);
  return stillwindClsx(...values);
}

/**
 * Creates a CVA selector that validates external classes and selected fragment channels.
 * The selector throws TypeError for inputs rejected by `stillwindClsxChecked` or `mergeFragmentsChecked`.
 *
 * @param options - Framework conventions and the keys of the runtime that created the fragments.
 * @param base - Compiled styles included in every selection.
 * @param config - Immutable compiled option maps, defaults, and compound conditions.
 * @returns A selector with the same selection and props behavior as `createCva`.
 */
export function createCvaChecked(
  options: FragmentRuntimeOptions,
  base?: CvaValue,
  config?: CompiledCvaConfig,
) {
  return createCva(options, base, config, stillwindClsxChecked, mergeFragmentsChecked);
}

/**
 * Validates fragment channels before materializing framework props.
 *
 * @param options - The receiving framework's conventions and distinct fragment keys.
 * @param values - Inputs accepted by `mergeFragments`.
 * @returns The same props as `mergeFragments` for valid inputs.
 * @throws TypeError for foreign symbol keys, unconverted class props, or a non-null class channel that is not a string.
 */
export function mergeFragmentsChecked(
  options: FragmentRuntimeOptions,
  values: readonly unknown[],
): Record<string, unknown> {
  const [, , stillwindClassKey, stillwindStyleKey, stillwindCustomPropertiesKey] = options;
  const keys = [stillwindClassKey, stillwindStyleKey, stillwindCustomPropertiesKey];

  function check(value: unknown): void {
    if (Array.isArray(value)) {
      value.forEach(check);
      return;
    }
    if (!isObject(value)) {
      return;
    }
    const symbols = Object.getOwnPropertySymbols(value);
    if (symbols.some((key) => !keys.includes(key))) {
      throw new TypeError("Stillwind received a fragment from a different runtime instance.");
    }
    if (
      !symbols.some((key) => keys.includes(key)) &&
      (Object.hasOwn(value, "class") || Object.hasOwn(value, "className"))
    ) {
      throw new TypeError(
        "Stillwind received a materialized props object; convert it with the ToFragment helper of its output format first.",
      );
    }
    const classes = value[stillwindClassKey];
    if (classes != null && typeof classes !== "string") {
      throw new TypeError("Stillwind received a fragment with a non-string class value.");
    }
  }

  values.forEach(check);
  return mergeFragments(options, values);
}

/**
 * Validates a composition base before converting it to a class-only fragment.
 *
 * @param options - The receiving framework's conventions and distinct fragment keys.
 * @param value - A non-null props object or a fragment belonging to this runtime.
 * @returns The result of `toFragment` for the object.
 * @throws TypeError when the value is null or is not an object.
 */
export function toFragmentChecked(
  options: FragmentRuntimeOptions,
  value: unknown,
): Record<PropertyKey, unknown> {
  if (!isObject(value)) {
    throw new TypeError("Stillwind expected a materialized props object.");
  }
  return toFragment(options, value);
}

/**
 * Names a keyed view transition after reading its token once.
 *
 * @param value - A nonempty class token or a record whose `.v` contains that token.
 * @param key - A key appended after a hyphen using `String`.
 * @returns The token and key escaped as one CSS identifier.
 * @throws TypeError when the token is missing, empty, or not a string.
 */
export function stillwindViewTransitionNameChecked(value: unknown, key: string | number): string {
  const token = typeof value === "string" ? value : isObject(value) ? value.v : undefined;
  if (typeof token !== "string" || !token) {
    throw new TypeError("Stillwind received a view transition without a class token.");
  }
  return stillwindViewTransitionName(token, key);
}

/** Checks fragment ownership and class contributions before materializing `class` and object styles. */
export function stillwindClassObjectMergeChecked(
  ...values: readonly unknown[]
): Record<string, unknown> {
  return mergeFragmentsChecked(classObjectOptions, values);
}

/** Rejects nonobjects before converting either class prop to a class-only fragment. */
export function stillwindClassObjectToFragmentChecked(
  value: unknown,
): Record<PropertyKey, unknown> {
  return toFragmentChecked(classObjectOptions, value);
}

/** Checks external classes and selected fragments before materializing CVA props with `class` and object styles. */
export function stillwindClassObjectCvaChecked(base?: CvaValue, config?: CompiledCvaConfig) {
  return createCvaChecked(classObjectOptions, base, config);
}

/** Checks fragment ownership and class contributions before materializing `class` and string styles. */
export function stillwindClassStringMergeChecked(
  ...values: readonly unknown[]
): Record<string, unknown> {
  return mergeFragmentsChecked(classStringOptions, values);
}

/** Rejects nonobjects before converting either class prop to a class-only fragment. */
export function stillwindClassStringToFragmentChecked(
  value: unknown,
): Record<PropertyKey, unknown> {
  return toFragmentChecked(classStringOptions, value);
}

/** Checks external classes and selected fragments before materializing CVA props with `class` and string styles. */
export function stillwindClassStringCvaChecked(base?: CvaValue, config?: CompiledCvaConfig) {
  return createCvaChecked(classStringOptions, base, config);
}

/** Checks fragment ownership and class contributions before materializing `className` and object styles. */
export function stillwindClassNameObjectMergeChecked(
  ...values: readonly unknown[]
): Record<string, unknown> {
  return mergeFragmentsChecked(classNameObjectOptions, values);
}

/** Rejects nonobjects before converting either class prop to a class-only fragment. */
export function stillwindClassNameObjectToFragmentChecked(
  value: unknown,
): Record<PropertyKey, unknown> {
  return toFragmentChecked(classNameObjectOptions, value);
}

/** Checks external classes and selected fragments before materializing CVA props with `className` and object styles. */
export function stillwindClassNameObjectCvaChecked(base?: CvaValue, config?: CompiledCvaConfig) {
  return createCvaChecked(classNameObjectOptions, base, config);
}
