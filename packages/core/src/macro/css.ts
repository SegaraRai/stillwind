import type { StyleVariant } from "./values.ts";

/**
 * Static utility input of `style.css()`: utility strings, arrays, and variant groups.
 *
 * `false`, `null`, and `undefined` contribute nothing. Attributes, custom properties,
 * external classes, refs, conditions, and other runtime values are unsupported,
 * because the call is replaced by a string at compile time.
 */
export type StyleCssInput =
  | string
  | false
  | null
  | undefined
  | readonly StyleCssInput[]
  | StyleCssGroup;

/**
 * A variant group in `style.css()`. Each key ending in `:` applies its variants to the
 * nested inputs, and the keys `""` and `":"` add none.
 */
export interface StyleCssGroup {
  /** Inputs that retain the enclosing variants. */
  readonly ""?: StyleCssInput;
  /** Variant prefixes with a final `:` for the nested inputs. */
  readonly [variant: StyleVariant]: StyleCssInput;
}

/**
 * Options of `style.css()`, written as a string literal object in the last argument.
 */
export interface StyleCssOptions {
  /**
   * Name of the `@stillwind-css-template` that places the output. Defaults to
   * `"default"`.
   */
  readonly template?: string;
  /**
   * Selector of the element that a utility styles when its own variants select no
   * element, such as `":host"`. A variant such as `[button]:` or `host:` still selects
   * its own element. Without this option, such utilities are omitted.
   */
  readonly selector?: string;
}
