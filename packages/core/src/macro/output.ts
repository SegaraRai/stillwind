/** Framework class and inline style props inferred from style inputs. */

import type { StyleArgument, StyleVariantValue } from "./input.ts";
import type { StyleKeyframesUse } from "./keyframes.ts";
import type { StyleRefProjection } from "./projection.ts";
import type { StyleRef, StyleVariantKey } from "./refs.ts";
import type { StyleVariant } from "./values.ts";
import type { StyleViewTransition, StyleViewTransitionUse } from "./view-transition.ts";

/**
 * Materialized framework props returned by a completed `style()` call.
 *
 * Store, export, pass, or return this value, spread it onto an element, or pass it to
 * another `style()` call to compose class-only results. The types reject results with
 * inline style as composition bases; at runtime such a base contributes only its class.
 * Directly nesting `style()` calls is rejected. Store the completed result before
 * composing it. `Props` describes the framework's class and style props and `Id` tracks
 * compiler identity.
 */
export type StyleResult<Id extends string = string, Props = object> = Props & {
  /**
   * Internal use only. The checker tracks identity through this field. Compiled props
   * contain no such attribute.
   *
   * @internal
   */
  readonly "data-__stillwindStyleIdentity": Id;
};

type HasInlineStyle<Input> = StyleArgument extends Input
  ? true
  : StyleVariantValue extends Input
    ? true
    : Input extends readonly unknown[]
      ? HasInlineStyle<Input[number]>
      : Input extends StyleResult
        ? "style" extends keyof Input
          ? true
          : false
        : Input extends StyleViewTransition
          ? false
          : Input extends StyleViewTransitionUse
            ? true
            : Input extends StyleRef | StyleRefProjection | StyleKeyframesUse | string
              ? false
              : Input extends object
                ? Extract<keyof Input, `--${string}`> extends never
                  ? HasInlineStyle<Input[Extract<keyof Input, "" | StyleVariant | StyleVariantKey>]>
                  : true
                : false;

export type StyleOutput<Props, Inputs extends readonly unknown[]> = Pick<
  Props,
  Extract<keyof Props, "class" | "className">
> &
  (true extends HasInlineStyle<Inputs[number]>
    ? Pick<Props, Extract<keyof Props, "style">>
    : object);
