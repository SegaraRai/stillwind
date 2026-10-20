/** Identity markers and rule syntax for ref projections. */

export declare const refProjection: unique symbol;

export declare const refIdentity: unique symbol;

/**
 * Input syntax returned by `ref.asAnchor()`, `ref.asContainer()`,
 * `ref.positionAnchor()`, or `style.anchorScope()`.
 *
 * Use the call directly in `style()` as a rule input, including an array, conditional
 * branch, or variant value at that position. It has no independently materialized value
 * and cannot be stored, exported, passed, or returned for later use. It does not attach
 * a ref marker.
 */
export type StyleRefProjection<RefId extends string = string> = {
  /** @internal */
  readonly [refProjection]: true;
  /** @internal */
  readonly [refIdentity]: {
    readonly ref: RefId;
  };
};
