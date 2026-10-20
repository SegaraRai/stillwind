/** Rule and prop inputs, with checks for the positions that accept them. */

import type { StyleKeyframesUse } from "./keyframes.ts";
import type { StyleResult } from "./output.ts";
import type { StyleRefProjection } from "./projection.ts";
import type { StyleRef, StyleVariantKey } from "./refs.ts";
import type { StillwindClassValue, StyleCustomPropertyValue, StyleVariant } from "./values.ts";
import type { StyleViewTransition, StyleViewTransitionUse } from "./view-transition.ts";

/**
 * Inputs allowed inside a variant wrapper in `style()`.
 *
 * Arrays and nested wrappers compose. False, null, and undefined contribute no rules.
 * Projection and animation-use calls must be written directly in the wrapper. The type
 * does not permit storing those calls for later use. Ref attachments, props
 * composition, external classes, and inline custom properties require positions outside
 * variant wrappers.
 */
export type StyleVariantValue =
  | string
  | StyleRefProjection
  | StyleViewTransition
  | StyleViewTransitionUse
  | StyleKeyframesUse
  | false
  | null
  | undefined
  | readonly StyleVariantValue[]
  | StyleVariantObject;

/**
 * A variant wrapper written directly in `style()`.
 *
 * The wrapper itself is input syntax and cannot be stored for later consumption.
 * Each key applies its conditions to every nested rule, from outermost to innermost.
 * The keys `""` and `":"` retain the enclosing conditions without adding variants.
 */
export interface StyleVariantObject {
  /** Rule inputs that retain the enclosing variants. */
  readonly ""?: StyleVariantValue;
  /**
   * Static variant prefixes with a final `:` wrap the associated rule inputs.
   */
  readonly [variant: StyleVariant]: StyleVariantValue;
  /**
   * Direct ref selectors, scope chains, and container queries provide computed keys
   * for the associated rules.
   */
  readonly [variant: StyleVariantKey]: StyleVariantValue;
}

/**
 * Custom-property entries for an object written directly in `style()` outside variant
 * wrappers. Names start with `--`, and values may be dynamic pure expressions.
 *
 * Store individual values or completed style props for reuse. A stored input object
 * does not replace the macro's direct object syntax.
 */
export type StyleCustomProperties = Readonly<Record<`--${string}`, StyleCustomPropertyValue>>;

// Keep computed ref-variant objects valid when TypeScript widens their keys to string.
type StyleObject = (StyleVariantObject | (StyleVariantObject & Partial<StyleCustomProperties>)) & {
  readonly class?: StillwindClassValue;
};

/**
 * Inputs accepted by `style()`, including recursive arrays and conditional branches.
 *
 * Completed style props and declaration values can be stored and passed for reuse.
 * Utility strings and input objects require static syntax. Projection and animation
 * uses require direct calls, and ref variants require computed keys or utility template
 * substitutions. This union does not make every member a portable value.
 */
export type StyleArgument =
  | StyleResult<string, object>
  | string
  | false
  | null
  | undefined
  | readonly StyleArgument[]
  | StyleObject
  | StyleRef
  | StyleRefProjection
  | StyleVariantKey
  | StyleKeyframesUse
  | StyleViewTransition
  | StyleViewTransitionUse;

declare const utilityStringCompletion: unique symbol;

// TypeScript's string literal completions skip intersection types.
type UtilityString = string & { readonly [utilityStringCompletion]?: never };

// Open string patterns allow an empty Record and must retain their contextual type
// so template expressions with ref substitutions remain assignable.
type CheckedString<Input extends string> = {} extends Record<Input, never> ? Input : UtilityString;

type CheckedVariantInput<Input> = StyleVariantValue extends Input
  ? StyleVariantValue
  : Input extends readonly unknown[]
    ? { [Key in keyof Input]: CheckedVariantInput<Input[Key]> }
    : Input extends
          | string
          | false
          | null
          | undefined
          | StyleRefProjection
          | StyleKeyframesUse
          | StyleViewTransition
          | StyleViewTransitionUse
      ? Input extends string
        ? CheckedString<Input>
        : StyleVariantValue
      : Input extends object
        ? {
            [Key in keyof Input]: string extends Key
              ? StyleVariantValue
              : Key extends "" | StyleVariant | StyleVariantKey
                ? CheckedVariantInput<Input[Key]>
                : never;
          }
        : StyleVariantValue;

type StyleBaseProperties<Input> = Input extends StyleResult ? Extract<keyof Input, "style"> : never;

type CheckedStyleInput<Input> = StyleArgument extends Input
  ? StyleArgument
  : [StyleBaseProperties<Input>] extends [never]
    ? CheckedClassInput<Input>
    : never;

type CheckedClassInput<Input> = Input extends readonly unknown[]
  ? { [Key in keyof Input]: CheckedStyleInput<Input[Key]> }
  : Input extends StyleResult
    ? StyleArgument
    : Input extends
          | StyleRef
          | StyleRefProjection
          | StyleKeyframesUse
          | StyleViewTransition
          | StyleViewTransitionUse
          | string
          | false
          | null
          | undefined
      ? Input extends string
        ? CheckedString<Input>
        : StyleArgument
      : Input extends (...args: never[]) => unknown
        ? never
        : Input extends object
          ? {
              [Key in keyof Input]: string extends Key
                ? StyleVariantValue
                : Key extends "" | StyleVariant | StyleVariantKey
                  ? CheckedVariantInput<Input[Key]>
                  : Key extends `--${string}`
                    ? StyleCustomPropertyValue
                    : Key extends "class"
                      ? StillwindClassValue
                      : never;
            }
          : StyleArgument;

export type CheckedStyleInputs<Inputs extends readonly unknown[]> = {
  [Key in keyof Inputs]: CheckedStyleInput<Inputs[Key]>;
};
