/** View transition declarations and participant naming used by style(). */

import type { refProjection } from "./projection.ts";

declare const viewTransitionIdentity: unique symbol;

/**
 * Participant naming syntax returned by `transition.for()`.
 *
 * Write the call directly as a rule input in `style()`, including an array, conditional
 * branch, or variant value at that position. The result has no independently
 * materialized value and cannot be stored or passed for later use.
 */
export type StyleViewTransitionUse<Id extends string = string> = {
  /** @internal */
  readonly [refProjection]: true;
  /** @internal */
  readonly [viewTransitionIdentity]: Id;
};

/**
 * A reusable view transition declaration created by `style.viewTransition()`.
 *
 * Store, export, pass, return, or conditionally select the declaration value.
 * Passing it to `style()` adds its transition class. Calling `.for()` also names the
 * participant. Calls to `.for()` must be consumed directly inside `style()`.
 * `Id` carries compiler identity and should be inferred by the checker.
 */
export type StyleViewTransition<Id extends string = string> = {
  /** @internal */
  readonly [refProjection]: true;
  /** @internal */
  readonly [viewTransitionIdentity]: Id;

  /**
   * Names participants by combining this declaration's class token with a key.
   *
   * The key must obey the purity contract of `style()`. Use this call directly
   * in a supported style input. A literal key produces a name in the stylesheet. A
   * dynamic key writes view-transition-name to inline style unless CSS variants need a
   * custom property to control when the name applies.
   *
   * The returned use cannot be stored or passed for later use. Simultaneously rendered
   * participants need distinct, application-stable keys.
   *
   * @param key - A string or number that forms the pairing name with the
   * declaration's class token. Omitting it uses the empty string.
   * @returns Rule input that supplies view-transition-class and view-transition-name.
   */
  for(key?: string | number): StyleViewTransitionUse<Id>;
};

/**
 * A static object accepted directly by `style.viewTransition()`.
 *
 * Supply at least one phase as a string literal. Phase strings may use media and
 * supports variants. Selector and container variants are rejected.
 * Store and transport the resulting declaration rather than this definition object.
 */
export type StyleViewTransitionDefinition = {
  /**
   * Optional string literal included in the generated transition class token.
   */
  label?: string;
  /**
   * Literal animation-option utilities for `::view-transition-group`.
   */
  group?: string;
  /**
   * Literal utilities for `::view-transition-old`.
   */
  old?: string;
  /**
   * Literal utilities for `::view-transition-new`.
   */
  new?: string;
  /**
   * Literal utilities for `::view-transition-image-pair`.
   */
  imagePair?: string;
};
