/** Framework props inferred from style inputs, including conditional attributes and inline style. */

import type { StyleArgument, StyleVariantValue } from "./input.ts";
import type { StyleKeyframesUse } from "./keyframes.ts";
import type { StyleRefProjection } from "./projection.ts";
import type { StyleRef, StyleVariantKey } from "./refs.ts";
import type { StyleVariant } from "./values.ts";
import type { StyleViewTransition, StyleViewTransitionUse } from "./view-transition.ts";

/**
 * Attribute names accepted by `style()` objects. Attribute values remain props.
 */
export type StyleAttributeName = `aria-${string}` | `data-${string}`;

/**
 * Materialized framework props returned by a completed `style()` call.
 *
 * Store, export, pass, or return this value, spread it onto an element, or pass it to
 * another `style()` call to compose class-only results. The types reject results with
 * attributes or inline style as composition bases; at runtime such a base contributes
 * only its class. Directly nesting `style()` calls is
 * rejected. Store the completed result before composing it. `Props` describes the
 * framework's attributes and `Id` tracks compiler identity.
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

type OutputKeys<Props> = Props extends unknown ? keyof Props : never;

// Keep absence separate from an attribute whose emitted value is undefined.
type OutputField<Value, Required extends boolean> = { value: Value; required: Required };

type OutputFieldAt<Props, Key extends PropertyKey> = Props extends unknown
  ? Key extends keyof Props
    ? Props[Key]
    : never
  : never;

type OutputValue<Props, Key extends PropertyKey> = [OutputFieldAt<Props, Key>] extends [never]
  ? never
  : OutputFieldAt<Props, Key> extends OutputField<infer Value, boolean>
    ? Value
    : never;

type RequiredOutputKeys<Props> = {
  [Key in OutputKeys<Props>]: [Props] extends [Record<Key, OutputField<unknown, true>>]
    ? Key
    : never;
}[OutputKeys<Props>];

type CombinedOutput<Props> = {
  [Key in OutputKeys<Props>]: OutputField<
    OutputValue<Props, Key>,
    Key extends RequiredOutputKeys<Props> ? true : false
  >;
};

type MergedOutputValue<Base, Next, Key extends PropertyKey> =
  Key extends RequiredOutputKeys<Next>
    ? OutputValue<Next, Key>
    : OutputValue<Base, Key> | OutputValue<Next, Key>;

type MergedOutput<Base, Next> = {
  [Key in OutputKeys<Base> | OutputKeys<Next>]: OutputField<
    MergedOutputValue<Base, Next, Key>,
    Key extends RequiredOutputKeys<Base> | RequiredOutputKeys<Next> ? true : false
  >;
};

type OutputFields<Props> = {
  [Key in keyof Props]-?: OutputField<
    Props[Key],
    Props extends Record<Key, unknown> ? true : false
  >;
};

type OptionalOutput<Props> = {
  [Key in keyof Props]: OutputField<OutputValue<Props, Key>, false>;
};

type MaterializedOutput<Props> = {
  [Key in RequiredOutputKeys<Props>]: OutputValue<Props, Key>;
} & {
  [Key in Exclude<OutputKeys<Props>, RequiredOutputKeys<Props>>]?: OutputValue<Props, Key>;
};

type AttributeOutputKeys<Input> = Exclude<
  Extract<keyof Input, StyleAttributeName>,
  `${string}:` | "data-__stillwindStyleIdentity"
>;

type InlineStyleOutput<Props> = OutputFields<Pick<Props, Extract<keyof Props, "style">>>;

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

type StyleInputOutput<Props, Input> = StyleArgument extends Input
  ? OutputFields<Props>
  : Input extends readonly unknown[]
    ? StyleInputsOutput<Props, Input>
    : Input extends StyleResult
      ? object
      : Input extends object
        ? OutputFields<Pick<Input, AttributeOutputKeys<Input>>> &
            (true extends HasInlineStyle<Input> ? InlineStyleOutput<Props> : object)
        : object;

type StyleInputsOutput<
  Props,
  Inputs extends readonly unknown[],
  Base = object,
> = Inputs extends readonly [infer First, ...infer Rest]
  ? StyleInputOutput<Props, First> extends infer Output
    ? StyleInputsOutput<Props, Rest, MergedOutput<Base, CombinedOutput<Output>>>
    : never
  : Inputs extends readonly []
    ? Base
    : StyleInputOutput<Props, Inputs[number]> extends infer Output
      ? MergedOutput<Base, OptionalOutput<CombinedOutput<Output>>>
      : never;

export type StyleOutput<Props, Inputs extends readonly unknown[]> = Pick<
  Props,
  Extract<keyof Props, "class" | "className">
> &
  MaterializedOutput<StyleInputsOutput<Props, Inputs>>;
