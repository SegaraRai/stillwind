/** Animation declarations and their direct use as style rules. */

import type { refProjection } from "./projection.ts";

declare const keyframesIdentity: unique symbol;

/**
 * Animation use syntax returned by calling a keyframes declaration with options.
 *
 * Write the call directly as a rule input in `style()`, including an array, conditional
 * branch, or variant value at that position. The result has no independently
 * materialized value and cannot be stored or passed for later use.
 */
export type StyleKeyframesUse<Id extends string = string> = {
  /** @internal */
  readonly [refProjection]: true;
  /** @internal */
  readonly [keyframesIdentity]: Id;
};

/**
 * A reusable animation declaration created by `style.keyframes()`.
 *
 * Store, export, pass, return, or conditionally select the declaration value.
 * Call it with literal animation options directly inside `style()` to apply it.
 * `Id` carries compiler identity and should be inferred by the checker.
 */
export type StyleKeyframes<Id extends string = string> = {
  /** @internal */
  readonly [keyframesIdentity]: Id;

  /**
   * Applies this animation with literal animation options, which are utilities of the
   * `animation` registry context.
   *
   * Use the call directly as a rule input in `style()`. The resulting animation use
   * cannot be stored or passed for later use.
   *
   * @param options - A string literal of options such as
   * `"duration-normal ease-standard both"`.
   * @returns Rule input that supplies animation-name and the requested options.
   */
  (options: string): StyleKeyframesUse<Id>;
};

/**
 * Static frames in a keyframes definition or a conditional replacement body.
 *
 * Write the object directly in `style.keyframes()`. A stored definition object is
 * rejected. Conditional variants must be media or supports conditions.
 */
export interface StyleKeyframesFrames {
  /**
   * `from`, `to`, or comma-separated unsigned integer percentage keys accept literal
   * utility strings without utility variants. Keys ending in `:` accept complete
   * replacement frame objects. Replacement bodies do not inherit the enclosing frames.
   */
  readonly [frameOrVariant: string]: string | StyleKeyframesFrames;
}

/**
 * A static object accepted directly by `style.keyframes()`, with frames and optional
 * conditional replacements. Each body must contain frames or replacement branches.
 *
 * Store and transport the resulting declaration rather than this definition object.
 */
export type StyleKeyframesDefinition = {
  /**
   * Optional string literal included in the generated animation name.
   */
  readonly label?: string;
} & StyleKeyframesFrames;
