/**
 * Shared types for the framework-specific `style` macros.
 *
 * Import `style` from a framework entrypoint such as `stillwind/react`.
 * Declaration values and completed style props can be stored and passed between
 * modules. Projection calls and ref variants must be consumed directly inside
 * `style()` at the positions described by their methods.
 *
 * @packageDocumentation
 */

import type {
  CvaClassProps,
  CvaConfig,
  CvaInput,
  CvaResult,
  CvaSelection,
  CvaVariants,
} from "./cva.ts";

export type * from "./cva.ts";

/**
 * Attribute names accepted by `style()` objects. Attribute values remain props.
 */
export type StyleAttributeName = `aria-${string}` | `data-${string}`;

/**
 * A static variant prefix used as an object key in `style()`, including its final `:`.
 * Nested keys compose from outermost to innermost. Raw selector keys must use
 * arbitrary-variant syntax, such as `[&:hover]:`.
 */
export type StyleVariant = `${string}:`;

/**
 * Ordinary values for the external-class entry `{ class: value }` in `style()`.
 *
 * Strings, numbers, and bigints become class text. Arrays are flattened, and object
 * keys are included when their values are truthy. Booleans, null, and undefined add no
 * class text. These values may be stored and passed to other functions.
 */
export type StillwindClassValue =
  | string
  | number
  | bigint
  | boolean
  | null
  | undefined
  | readonly StillwindClassValue[]
  | Readonly<Record<string, unknown>>;

declare const refProjection: unique symbol;
declare const refIdentity: unique symbol;
declare const refVariantKind: unique symbol;
declare const keyframesIdentity: unique symbol;
declare const viewTransitionIdentity: unique symbol;

/**
 * Materialized framework props returned by a completed `style()` call.
 *
 * Store, export, pass, or return this value, spread it onto an element, or pass it to
 * another `style()` call to compose styles. Directly nesting `style()` calls is
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

/**
 * Internal use only. The compiler inserts this identity argument in source used by the
 * TypeScript checker. Application code must not construct or pass it.
 *
 * @internal
 */
export type StillwindDeclarationIdentity<Id extends string> = {
  readonly __stillwindDeclarationIdentity: Id;
};

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

/**
 * An explicit scope written as a literal keyword or a literal array of refs.
 *
 * For a ref that can resolve to several identities, the scope includes all of them,
 * regardless of which one is selected at runtime. Duplicate identities and empty arrays
 * produce lint findings.
 *
 * Write the keyword or array directly in `asAnchor()` or `style.anchorScope()`.
 * A stored scope value does not satisfy the compiler's literal syntax contract.
 */
export type AnchorScope = "all" | "none" | readonly StyleRef[];

/**
 * CSS-wide keywords accepted as literal `container-type` values by `asContainer`.
 *
 * Write the keyword directly as a string literal in the macro call.
 * A stored string does not satisfy that syntax contract.
 */
export type CssWideKeyword =
  | "initial"
  | "inherit"
  | "unset"
  | "revert"
  | "revert-layer"
  | "revert-rule";

/**
 * Literal `container-type` values accepted by `asContainer`.
 *
 * A size mode may combine with `scroll-state` in either listed order. Write the string
 * literal directly in the call. A stored string is rejected.
 */
export type ContainerType =
  | CssWideKeyword
  | "normal"
  | "size"
  | "inline-size"
  | "scroll-state"
  | "size scroll-state"
  | "scroll-state size"
  | "inline-size scroll-state"
  | "scroll-state inline-size";

/**
 * A DOM identity attached by passing the ref itself to `style()`.
 *
 * `asAnchor` and `asContainer` add CSS that applies only when the same element has this
 * ref's marker. A ref declaration can also supply a scope to `.in()`. The declaration
 * value may be stored, exported, passed, returned, and selected conditionally. Its
 * projection and variant calls must appear directly inside `style()` and cannot be kept
 * as separate values. `RefId` carries compiler identity. Application code should let
 * the checker infer them.
 */
export type StyleRef<RefId extends string = string> = {
  /** @internal */
  readonly [refIdentity]: {
    readonly ref: RefId;
  };

  /**
   * Supplies this ref's anchor-name and optional anchor-scope.
   *
   * If `scope` is an array, this ref is included only when listed in that array.
   * Use the call directly in a style input and attach the ref to the same element.
   * The returned projection cannot be stored or passed for later use.
   *
   * @param scope - A literal `"all"`, `"none"`, or array of refs. Empty arrays and
   * duplicate identities produce lint findings. An empty array or omitted argument
   * emits no scope declaration.
   * @returns Rule input that adds this ref's anchor-name when consumed by `style()`.
   */
  asAnchor(scope?: AnchorScope): StyleRefProjection<RefId>;

  /**
   * Adds this ref's container-name and, if provided, the literal container-type.
   *
   * Omitting `type` emits no container-type, leaving the CSS cascade to determine it.
   * Use the call directly in a style input and attach the ref to the same element.
   * The returned projection cannot be stored or passed for later use.
   *
   * @param type - An optional string literal from {@link ContainerType}.
   * @returns Rule input that adds this ref's container-name and optional container-type.
   */
  asContainer(type?: ContainerType): StyleRefProjection<RefId>;

  /**
   * Refers to this ref's generated anchor-name from the styled element.
   *
   * Supply an anchor provider with `asAnchor()` on an element carrying this ref.
   * Use the call directly as a rule input in `style()`. The returned projection cannot
   * be stored or passed for later use.
   *
   * @returns Rule input that adds position-anchor to the styled element.
   */
  positionAnchor(): StyleRefProjection<RefId>;

  /**
   * Tests this ref's presence within the supplied scope. This is shorthand for calling
   * this ref without arguments and then applying `.in(scope)`.
   *
   * Use the complete chain directly as a computed variant key or a variant segment
   * followed by `:` in a utility template inside `style()`.
   *
   * @param scope - A raw ref declaration or an unscoped ref call or static tag.
   * The scope ref must be a strict ancestor of both this ref and the styled target.
   * Any variants on the scope operand test that scope element.
   * @returns Scoped selector syntax that accepts further `.in()` calls.
   * The result cannot be stored or passed for later use.
   */
  in<const ScopeId extends string>(
    scope: RefScope<ScopeId>,
  ): ScopedRefVariant<RefId, readonly [ScopeId]>;

  /**
   * Selects this ref's named container without a query condition.
   *
   * Supply its container-name with `asContainer()` on an element carrying this ref.
   * Use the call directly as a computed variant key or a complete variant segment
   * followed by `:` in a utility template inside `style()`.
   *
   * @returns Variant syntax that emits `@container <name>` without parentheses.
   * The result cannot be stored or passed as an independent value.
   */
  at(): ContainerVariant<RefId>;

  /**
   * Applies a literal CSS query to this ref's named container.
   *
   * Supply its container-name with `asContainer()` on an element carrying this ref.
   * Use the call directly as a computed variant key or a complete variant segment
   * followed by `:` in a utility template inside `style()`.
   *
   * @param query - A string literal containing the CSS condition without its outer
   * parentheses. Empty text emits only the container name.
   * @returns Variant syntax for the named container query. The result cannot be
   * stored or passed as an independent value.
   */
  at(query: string): ContainerVariant<RefId>;

  /**
   * Applies a static tagged query to this ref's named container.
   *
   * Supply its container-name with `asContainer()` on an element carrying this ref.
   * Use the tag directly as a computed variant key or a complete variant segment
   * followed by `:` in a utility template inside `style()`.
   *
   * @param strings - Text from a tag such as ``ref.at`width >= 40rem` ``.
   * Interpolations are rejected. Empty text emits only the container name.
   * @returns Variant syntax for the named container query. The result cannot be
   * stored or passed as an independent value.
   */
  at(strings: TemplateStringsArray): ContainerVariant<RefId>;

  /**
   * Tests a static variant sequence on the referenced element.
   *
   * Use the tag directly as a computed variant key or a complete variant segment
   * followed by `:` in a utility template inside `style()`. Variants outside the
   * tag apply to the styled target.
   *
   * @param strings - Text from a tag such as ``ref`hover:active` ``. Its final `:`
   * is optional. Interpolations are rejected. Empty text tests ref presence.
   * @returns Ref variant syntax. The result cannot be stored or passed as an
   * independent value.
   */
  (strings: TemplateStringsArray): RefVariant<RefId>;

  /**
   * Tests ref presence without a state condition. Without `.in()`, this ref must be an
   * ancestor of the styled target. `.in()` scopes the selector explicitly.
   *
   * Use the call directly as a computed variant key or a complete variant segment
   * followed by `:` in a utility template inside `style()`.
   *
   * @returns Ref variant syntax. The result cannot be stored or passed as an
   * independent value.
   */
  (): RefVariant<RefId>;

  /**
   * Tests a literal variant sequence on the referenced element.
   *
   * Use the call directly as a computed variant key or a complete variant segment
   * followed by `:` in a utility template inside `style()`. Variants outside the
   * call apply to the styled target.
   *
   * @param variants - A string literal such as `"hover:active"`, with an optional
   * final `:`. Empty text tests ref presence.
   * @returns Ref variant syntax. The result cannot be stored or passed as an
   * independent value.
   */
  (variants: string): RefVariant<RefId>;
};

/**
 * A scope operand for `.in()`: a raw ref declaration or an unscoped ref call or static
 * tag. Scoped selectors and container queries cannot supply a scope.
 *
 * Declaration values may be reused. Calls and tags must appear directly in the
 * `.in()` chain consumed by `style()`.
 * `RefId` preserves the possible declaration identities of the scope operand.
 */
export type RefScope<RefId extends string = string> = StyleRef<RefId> | RefVariant<RefId>;

type StyleVariantKey = string & {
  readonly [refProjection]: true;
  readonly at: never;
};

/**
 * An unscoped ref selector returned by a ref call or static tag.
 *
 * Without `.in()`, the referenced element must be an ancestor of the styled target.
 * Use the complete call, tag, or `.in()` chain directly as a computed variant key or a
 * variant segment followed by `:` in a utility template inside `style()`.
 * This syntax cannot be stored, exported, passed, or returned for later use.
 * `RefId` preserves the possible identities of the referenced element.
 */
export type RefVariant<RefId extends string = string> = StyleVariantKey & {
  /** @internal */
  readonly [refVariantKind]: "unscoped";
  /** @internal */
  readonly [refIdentity]: {
    readonly ref: RefId;
  };

  /**
   * Scopes this selector to a strict ancestor of the referenced element and the styled
   * target. Any variants on the scope operand test that scope element.
   *
   * Use the complete chain directly as a computed key or utility variant segment inside
   * `style()`.
   *
   * @param scope - A raw ref declaration or an unscoped ref call or static tag.
   * @returns Scoped selector syntax that accepts further `.in()` calls.
   * The result cannot be stored or passed for later use.
   */
  in<const ScopeId extends string>(
    scope: RefScope<ScopeId>,
  ): ScopedRefVariant<RefId, readonly [ScopeId]>;
};

/**
 * A ref selector with one or more explicit ancestor scopes.
 *
 * Each further `.in()` adds an independent condition that its scope contains both the
 * referenced element and the styled target. The method may be chained any number of
 * times. Use the complete chain directly as a computed key or a variant segment
 * followed by `:` in a utility template inside `style()`.
 *
 * This syntax cannot be stored, exported, passed, or returned for later use.
 * `RefId` preserves the referenced identities. `ScopeIds` preserves each scope's
 * possible identities in the order of the `.in()` calls.
 */
export type ScopedRefVariant<
  RefId extends string = string,
  ScopeIds extends readonly string[] = readonly string[],
> = StyleVariantKey & {
  /** @internal */
  readonly [refVariantKind]: "scoped";
  /** @internal */
  readonly [refIdentity]: {
    readonly ref: RefId;
    readonly scopes: ScopeIds;
  };

  /**
   * Adds a scope that must be a strict ancestor of the referenced element and the
   * styled target. Each scope condition applies independently. Any variants on
   * the operand test that scope element.
   *
   * Use the complete chain directly as a computed key or utility variant segment
   * inside `style()`.
   *
   * @param scope - A raw ref declaration or an unscoped ref call or static tag.
   * @returns Scoped selector syntax that preserves the existing scopes and accepts
   * further `.in()` calls. The result cannot be stored or passed for later use.
   */
  in<const ScopeId extends string>(
    scope: RefScope<ScopeId>,
  ): ScopedRefVariant<RefId, readonly [...ScopeIds, ScopeId]>;
};

/**
 * A named container query returned by a raw ref's `.at()` call or static tag.
 * Supply the container name with `.asContainer()` on an element carrying that ref.
 *
 * Use the call or tag directly as a computed key or a variant segment followed by
 * `:` in a utility template inside `style()`. This syntax cannot be stored,
 * exported, passed, or returned for later use. It accepts no further macro methods.
 * `RefId` preserves the possible identities of the container provider.
 */
export type ContainerVariant<RefId extends string = string> = StyleVariantKey & {
  /** @internal */
  readonly [refVariantKind]: "container";
  /** @internal */
  readonly [refIdentity]: {
    readonly ref: RefId;
  };
};

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
   * Applies this animation with literal animation-option utilities.
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
   * The key must obey the purity contract of {@link StyleMacro}. Use this call directly
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

/**
 * Inputs allowed inside a variant wrapper in `style()`.
 *
 * Arrays and nested wrappers compose. False, null, and undefined contribute no rules.
 * Projection and animation-use calls must be written directly in the wrapper. The type
 * does not permit storing those calls for later use. Ref attachments, props
 * composition, attributes, external classes, and inline custom properties require
 * positions outside variant wrappers.
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
 */
export interface StyleVariantObject {
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
 * Ordinary values for inline custom properties.
 *
 * These may be stored and passed between functions. Strings are CSS text, numbers have
 * no unit, and URL objects become escaped `url("...")` values. True becomes one space,
 * and false becomes `initial`.
 */
export type StyleCustomPropertyValue = URL | string | number | boolean;

/**
 * Custom-property entries for an object written directly in `style()` outside variant
 * wrappers. Names start with `--`, and values may be dynamic pure expressions.
 *
 * Store individual values or completed style props for reuse. A stored input object
 * does not replace the macro's direct object syntax.
 */
export type StyleCustomProperties = Readonly<Record<`--${string}`, StyleCustomPropertyValue>>;

// Keep computed ref-variant objects valid when TypeScript widens their keys to string.
type StyleObject = Partial<Record<StyleAttributeName, unknown>> &
  (StyleVariantObject | (StyleVariantObject & Partial<StyleCustomProperties>)) & {
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
                  ? HasInlineStyle<Input[Extract<keyof Input, StyleVariant | StyleVariantKey>]>
                  : true
                : false;

type StyleInputOutput<Props, Input> = StyleArgument extends Input
  ? OutputFields<Props>
  : Input extends readonly unknown[]
    ? StyleInputsOutput<Props, Input>
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

type CheckedVariantInput<Input> = StyleVariantValue extends Input
  ? StyleVariantValue
  : Input extends readonly unknown[]
    ? { [Key in keyof Input]: CheckedVariantInput<Input[Key]> }
    : Input extends string | false | null | undefined | StyleRefProjection | StyleKeyframesUse
      ? StyleVariantValue
      : Input extends object
        ? {
            [Key in keyof Input]: string extends Key
              ? StyleVariantValue
              : Key extends StyleVariant | StyleVariantKey
                ? CheckedVariantInput<Input[Key]>
                : never;
          }
        : StyleVariantValue;

type CheckedStyleInput<Input> = StyleArgument extends Input
  ? StyleArgument
  : Input extends readonly unknown[]
    ? { [Key in keyof Input]: CheckedStyleInput<Input[Key]> }
    : Input extends
          | StyleResult
          | StyleRef
          | StyleRefProjection
          | StyleKeyframesUse
          | StyleViewTransition
          | StyleViewTransitionUse
          | string
          | false
          | null
          | undefined
      ? StyleArgument
      : Input extends (...args: never[]) => unknown
        ? never
        : Input extends object
          ? {
              [Key in keyof Input]: string extends Key
                ? StyleVariantValue
                : Key extends StyleVariant | StyleVariantKey
                  ? CheckedVariantInput<Input[Key]>
                  : Key extends StyleAttributeName
                    ? unknown
                    : Key extends `--${string}`
                      ? StyleCustomPropertyValue
                      : Key extends "class"
                        ? StillwindClassValue
                        : never;
            }
          : StyleArgument;

type CheckedStyleInputs<Inputs extends readonly unknown[]> = {
  [Key in keyof Inputs]: CheckedStyleInput<Inputs[Key]>;
};

type StyleOutput<Props, Inputs extends readonly unknown[]> = Pick<
  Props,
  Extract<keyof Props, "class" | "className">
> &
  MaterializedOutput<StyleInputsOutput<Props, Inputs>>;

/**
 * Compile-time UI declarations with pure value expressions.
 *
 * Only completed style calls and declaration macros produce reusable values.
 * Projections require their supported style input positions. Ref variants require
 * computed variant keys or utility template substitutions.
 *
 * Each argument expression must return an equivalent value for the same inputs and must
 * not mutate shared data or perform observable effects, including through callees,
 * getters, or value conversion. Calls need no PURE annotation. Functions may mutate
 * fresh local data used only to compute their result.
 *
 * Evaluation order and count are not API guarantees: unused expressions may be omitted
 * and class/value conversions may be combined. Do not rely on exceptions from unused
 * expressions. Read time/randomness and perform I/O, state updates, Hooks, await/yield,
 * imports, JSX, and class initialization outside the macro. Pass their resulting
 * values. Reactive reads stay in the current computation scope and must not be used
 * only to establish a subscription.
 *
 * Names, utility definitions, animation options, and selector/query arguments are
 * static declarations. Dynamic identity selectors and view-transition keys obey the
 * same value-expression contract. Declaration-identity overloads are internal checker
 * metadata.
 */
export type StyleMacro<Props = object> = {
  /**
   * Internal use only. The checker supplies the final identity argument to track the
   * completed props value. Application code must use the overload without it.
   *
   * @internal
   */
  <const Inputs extends readonly StyleArgument[], Id extends string>(
    ...inputs: [...Inputs, StillwindDeclarationIdentity<Id>] &
      [...CheckedStyleInputs<Inputs>, StillwindDeclarationIdentity<Id>]
  ): StyleResult<Id, StyleOutput<Props, Inputs>>;

  /**
   * Compiles static utility branches and combines them with attributes, inline custom
   * properties, external classes, and completed style props.
   *
   * Value expressions must obey the purity contract of {@link StyleMacro}. Projection
   * and animation-use calls must appear directly at supported input positions. Ref
   * variants require computed keys or utility template substitutions.
   *
   * @param inputs - Rule and prop inputs in composition order. Later prop values
   * replace earlier values, and classes are concatenated. Utility construction must be
   * static, and invalid utilities or unsupported syntax cause compilation errors.
   * @returns Framework props that may be stored, exported, passed, returned, or spread
   * onto an element. Store this result before passing it to another `style()` call.
   * Direct nested calls are rejected. Attribute names and values follow the inputs, and
   * conditional attributes are optional. The class prop is optional. The optional style
   * prop is present in the type when inputs include custom properties, participant
   * naming, or composed props that may contain inline style. CSS is emitted at build
   * time.
   */
  <const Inputs extends readonly StyleArgument[]>(
    ...inputs: Inputs & CheckedStyleInputs<Inputs>
  ): StyleResult<string, StyleOutput<Props, Inputs>>;

  /**
   * Declares an unlabeled DOM identity in a binding initializer.
   * Passing the declaration to `style()` attaches its marker to that element.
   *
   * @returns A reusable ref declaration that may be stored, exported, passed, returned,
   * or conditionally selected. Its identity belongs to the declaration site. Repeated
   * evaluation does not allocate new CSS identities.
   */
  ref(): StyleRef;

  /**
   * Declares a labeled DOM identity in a binding initializer.
   * Passing the declaration to `style()` attaches its marker to that element.
   *
   * @param label - A string literal used in generated names. Empty text omits the
   * label. Labels do not determine declaration identity.
   * @returns A reusable ref declaration that may be stored, exported, passed, returned,
   * or conditionally selected. Repeated evaluation keeps its identity.
   */
  ref(label: string): StyleRef;

  /**
   * Internal use only. The checker supplies identity for an unlabeled ref declaration.
   * Application code must use `ref()` without this argument.
   *
   * @internal
   */
  ref<const Id extends string>(identity: StillwindDeclarationIdentity<Id>): StyleRef<Id>;

  /**
   * Internal use only. The checker supplies identity for a labeled ref declaration.
   * Application code must use `ref(label)` without the identity argument.
   *
   * @internal
   */
  ref<const Id extends string>(
    label: string,
    identity: StillwindDeclarationIdentity<Id>,
  ): StyleRef<Id>;

  /**
   * Declares a local animation in a binding initializer and assigns its CSS name.
   *
   * Apply it by calling the returned declaration with literal options directly inside
   * `style()`. The animation use cannot be stored for later consumption.
   *
   * @param definition - A direct static object with frame utility strings, optional
   * media/supports replacement bodies, and an optional literal label. Invalid frame
   * syntax or utilities cause compilation errors.
   * @returns A reusable keyframes declaration that may be stored, exported, passed,
   * returned, or conditionally selected. Its identity belongs to the declaration site.
   */
  keyframes(definition: StyleKeyframesDefinition): StyleKeyframes;

  /**
   * Internal use only. The checker supplies identity for a keyframes declaration.
   * Application code must use `keyframes(definition)` without the identity argument.
   *
   * @internal
   */
  keyframes<const Id extends string>(
    definition: StyleKeyframesDefinition,
    identity: StillwindDeclarationIdentity<Id>,
  ): StyleKeyframes<Id>;

  /**
   * Declares view transition phases in a binding initializer and assigns their class.
   *
   * Pass the declaration to `style()` to mark participants, or call `.for(key)`
   * directly inside `style()` to also name them. The naming use cannot be stored.
   *
   * @param definition - A direct static object with at least one literal phase
   * string and an optional literal label. Invalid phases, utilities, or condition
   * variants cause compilation errors.
   * @returns A reusable transition declaration that may be stored, exported, passed,
   * returned, or conditionally selected. Its identity belongs to the declaration site.
   */
  viewTransition(definition: StyleViewTransitionDefinition): StyleViewTransition;

  /**
   * Internal use only. The checker supplies identity for a view transition declaration.
   * Application code must use `viewTransition(definition)` without the identity argument.
   *
   * @internal
   */
  viewTransition<const Id extends string>(
    definition: StyleViewTransitionDefinition,
    identity: StillwindDeclarationIdentity<Id>,
  ): StyleViewTransition<Id>;

  /**
   * Adds anchor-scope without requiring a ref marker on the element.
   *
   * Use this call directly in a style input. The argument follows the same literal
   * keyword or array rules as `ref.asAnchor`. An empty array emits no declaration.
   * The returned projection cannot be stored, exported, passed, or returned for later
   * consumption.
   *
   * @param scope - A literal `"all"`, `"none"`, or array of refs. All possible
   * identities of each entry are included. Empty arrays and duplicate identities
   * produce lint findings.
   * @returns Rule input that supplies anchor-scope when consumed by `style()`.
   */
  anchorScope(scope: AnchorScope): StyleRefProjection;

  /**
   * Compiles a CVA definition and returns a function that selects element props.
   * Write the configuration and utility inputs directly in the call. Custom properties
   * remain inline and value expressions obey this macro's purity contract.
   *
   * @param base - Optional styles included in every selection.
   * @param config - Static variants, defaults, and compound conditions.
   * @returns A reusable factory returning fresh framework class and style props.
   * Its results cannot be passed to style() for composition.
   */
  cva<const Variants extends CvaVariants = Record<never, never>>(
    base?: CvaInput,
    config?: CvaConfig<Variants>,
  ): (props?: CvaSelection<Variants> & CvaClassProps) => CvaResult<Props>;
};

/**
 * Internal use only. Builds the placeholder exported by each framework entrypoint.
 * The compiler replaces supported macro calls before the application runs.
 *
 * @returns A fresh callable object with declaration and scope methods. Calling the
 * object or any of these methods throws an Error about missing integration. Creating
 * the placeholder itself does not execute any macro or generate CSS.
 *
 * @internal
 */
export function createStyleMacro<Props>(): StyleMacro<Props> {
  return Object.assign(
    (): never => {
      throw new Error(
        "Stillwind style() is a compile-time macro. Configure the Stillwind integration for your build tool.",
      );
    },
    {
      anchorScope(): never {
        throw new Error("Stillwind style.anchorScope() is a compile-time macro.");
      },
      ref(): never {
        throw new Error("Stillwind style.ref() is a compile-time macro.");
      },
      keyframes(): never {
        throw new Error("Stillwind style.keyframes() is a compile-time macro.");
      },
      viewTransition(): never {
        throw new Error("Stillwind style.viewTransition() is a compile-time macro.");
      },
      cva(): never {
        throw new Error("Stillwind style.cva() is a compile-time macro.");
      },
    },
  );
}
