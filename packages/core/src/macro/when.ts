/** Stored variant sequences created by `style.when()`. */

import type { StyleVariantKey } from "./refs.ts";

declare const storedVariantIdentity: unique symbol;

/**
 * A variant sequence stored by `style.when()`.
 *
 * Use the value as a computed variant key in `style()`, as a variant segment followed
 * by `:` in a utility template inside `style()`, or in the argument of another
 * `style.when()`. Each use means the same as writing the stored sequence at that
 * position. The value may be stored, exported, imported, and passed through bindings
 * that the compiler can resolve to one declaration. At runtime the value is
 * `undefined`. `Id` carries compiler identity and should be inferred by the checker.
 */
export type StyleStoredVariant<Id extends string = string> = StyleVariantKey & {
  /** @internal */
  readonly [storedVariantIdentity]: Id;
};
