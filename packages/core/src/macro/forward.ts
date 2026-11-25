/**
 * Props that a component received from its caller and passes on with `style.forward()`.
 */

import type { StillwindClassValue } from "./values.ts";

/**
 * The class and style props that `style.forward()` reads. Other props of the object are
 * ignored, so a component can pass all of its props.
 */
export type StyleForwardProps = {
  /** Class values, joined as `stillwindClsx` joins them. */
  readonly class?: StillwindClassValue;
  /** Class values, joined after `class` as `stillwindClsx` joins them. */
  readonly className?: StillwindClassValue;
  /**
   * Inline style in the form that the framework accepts: declaration text, an object, or,
   * in frameworks that accept them, an array of these values.
   */
  readonly style?: string | object | null;
};

declare const styleForwardBrand: unique symbol;

/**
 * Props that a caller forwarded. Pass the value directly to `style()` or as the second
 * argument of a `style.recipe()` factory. It cannot be stored for later use.
 */
export interface StyleForward {
  readonly [styleForwardBrand]: true;
}
