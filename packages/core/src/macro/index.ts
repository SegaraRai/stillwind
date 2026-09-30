/** Compile-time macro contracts shared by every framework entrypoint. */
export type StyleAttributeName = `aria-${string}` | `data-${string}`;

/** Augmented by the generated `src/stillwind.generated.d.ts` file. */
export interface StillwindUtilityVocabulary {}
export interface StillwindVariantVocabulary {}

type VocabularyUtility = Extract<keyof StillwindUtilityVocabulary, string>;
type VocabularyVariant = Extract<keyof StillwindVariantVocabulary, string>;
type UtilityWithVariant<Utility extends string, Variant extends string> = [Utility] extends [never]
  ? string
  : [Variant] extends [never]
    ? Utility
    : Utility | `${Variant}:${string}`;
/** Utility names and variant prefixes; the compiler validates the full token.
 * Avoid materializing every utility/variant pair in large generated vocabularies.
 */
export type StillwindUtility = UtilityWithVariant<VocabularyUtility, VocabularyVariant>;

// Utility lists are intentionally validated by the compiler. A recursive
// template-literal union for arbitrary-length class lists quickly exceeds
// TypeScript's union-complexity limit in real themes.
type UtilityString = string;

export type StyleVariant = `${string}:`;

/** Values accepted by the explicit `{ class: … }` external-class entry. */
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
declare const keyframesIdentity: unique symbol;
declare const viewTransitionIdentity: unique symbol;

export type StyleResult<Id extends string = string, Props = object> = Props & {
  /** Checker-only declaration identity; macro lowering never emits this attribute. */
  readonly "data-__stillwindStyleIdentity": Id;
};

export type StillwindDeclarationIdentity<Id extends string> = {
  readonly __stillwindDeclarationIdentity: Id;
};

/**
 * A ref's CSS projection, which must appear directly in a supported style input.
 * It cannot be stored for reuse and does not attach the receiver's marker.
 */
export type StyleRefProjection<RefId extends string = string, BoundaryId extends string = never> = {
  readonly [refProjection]: true;
  readonly [refIdentity]: { readonly ref: RefId; readonly boundary: BoundaryId };
};

/**
 * An explicit scope written as a literal keyword or a literal array of refs.
 * For a ref that can resolve to several identities, the scope includes all of them,
 * regardless of which one is selected at runtime. Duplicate identities and empty
 * arrays produce lint findings.
 */
export type AnchorScope = "all" | "none" | readonly StyleRef<string, string>[];
export type CssWideKeyword =
  | "initial"
  | "inherit"
  | "unset"
  | "revert"
  | "revert-layer"
  | "revert-rule";
/** Literal values accepted by `asContainer` for container-type, including CSS-wide keywords. */
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
 * `asAnchor` and `asContainer` add CSS that applies only when the same element has
 * this ref's marker. A ref declaration can also serve as the boundary for other refs.
 */
export type StyleRef<RefId extends string = string, BoundaryId extends string = never> = {
  readonly [refIdentity]: { readonly ref: RefId; readonly boundary: BoundaryId };
  /**
   * Adds this ref's anchor-name. Omitting `scope` emits no anchor-scope.
   * If `scope` is an array, this ref is included only when listed in that array.
   * Use the call directly in a style input and attach the ref to the same element.
   */
  asAnchor(scope?: AnchorScope): StyleRefProjection<RefId, BoundaryId>;
  /**
   * Adds this ref's container-name and, if provided, the literal container-type.
   * Omitting `type` emits no container-type, leaving the CSS cascade to determine it.
   * Use the call directly in a style input and attach the ref to the same element.
   */
  asContainer(type?: ContainerType): StyleRefProjection<RefId, BoundaryId>;
  /** Adds position-anchor to the styled element. Use this call directly as a style input. */
  positionAnchor(): StyleRefProjection<RefId, BoundaryId>;
  /** Creates a named-container query from literal CSS for a variant key or template substitution. */
  at(query: string): StyleRefVariant<RefId, BoundaryId>;
  at(strings: TemplateStringsArray): StyleRefVariant<RefId, BoundaryId>;
  (strings: TemplateStringsArray): StyleRefVariant<RefId, BoundaryId>;
  (): StyleRefVariant<RefId, BoundaryId>;
  (variants: string): StyleRefVariant<RefId, BoundaryId>;
};

/**
 * A ref selector or query used as a computed variant key or a substitution in a
 * utility template. Although its type extends string, it cannot be used as an
 * independent runtime value.
 */
export type StyleRefVariant<
  RefId extends string = string,
  BoundaryId extends string = never,
> = string & {
  readonly [refProjection]: true;
  readonly [refIdentity]: { readonly ref: RefId; readonly boundary: BoundaryId };
};

/** Must appear directly in a style input. It cannot be stored for reuse as a value. */
export type StyleKeyframesUse<Id extends string = string> = {
  readonly [refProjection]: true;
  readonly [keyframesIdentity]: Id;
};

export type StyleKeyframes<Id extends string = string> = {
  (options: string): StyleKeyframesUse<Id>;
  readonly [keyframesIdentity]: Id;
};

/** One frames object: `from`, `to`, or percentage keys, each a utility string or a conditional body. */
export interface StyleKeyframesFrames {
  readonly [frameOrVariant: string]: string | StyleKeyframesFrames;
}

/** A keyframes declaration: one frames object and the optional label its generated name spells out. */
export type StyleKeyframesDefinition = { readonly label?: string } & StyleKeyframesFrames;

/** Must appear directly in a style input. It cannot be stored for reuse as a value. */
export type StyleViewTransitionUse<Id extends string = string> = {
  readonly [refProjection]: true;
  readonly [viewTransitionIdentity]: Id;
};

/** A declared view transition: participants share its class, and only `.for` names them. */
export type StyleViewTransition<Id extends string = string> = {
  readonly [refProjection]: true;
  readonly [viewTransitionIdentity]: Id;
  /**
   * Names participants using a string or number key, which defaults to the empty string.
   * The key must obey the macro purity contract. Use this call directly in a supported
   * style input. A literal key produces a name in the stylesheet. A dynamic key writes
   * view-transition-name to inline style unless CSS variants need a custom property
   * to control when the name applies.
   */
  for(key?: string | number): StyleViewTransitionUse<Id>;
};

export type StyleViewTransitionDefinition = {
  /** Optional label the generated class token spells out; a literal string only. */
  label?: string;
  group?: string;
  old?: string;
  new?: string;
  imagePair?: string;
};

/** Variant wrappers compose from outermost to innermost, just like utility prefixes. */
export type StyleVariantValue =
  | UtilityString
  | StyleRefProjection<string, string>
  | StyleViewTransition
  | StyleViewTransitionUse
  | StyleKeyframesUse
  | false
  | null
  | undefined
  | readonly StyleVariantValue[]
  | StyleVariantObject;

export interface StyleVariantObject {
  readonly [variant: StyleVariant]: StyleVariantValue;
  readonly [variant: StyleRefVariant<string, string>]: StyleVariantValue;
}

/** Inline CSS values; strings are CSS text and URL objects become url("..."). */
export type StyleCustomPropertyValue = URL | string | number | boolean;
export type StyleCustomProperties = Readonly<Record<`--${string}`, StyleCustomPropertyValue>>;

// Keep computed ref-variant objects valid when TypeScript widens their keys to string.
type StyleObject = Partial<Record<StyleAttributeName, unknown>> &
  (StyleVariantObject | (StyleVariantObject & Partial<StyleCustomProperties>)) & {
    readonly class?: StillwindClassValue;
  };

export type StyleArgument =
  | StyleResult<string, object>
  | UtilityString
  | false
  | null
  | undefined
  | readonly StyleArgument[]
  | StyleObject
  | StyleRef<string, string>
  | StyleRefProjection<string, string>
  | StyleRefVariant<string, string>
  | StyleKeyframesUse
  | StyleViewTransition
  | StyleViewTransitionUse;

/**
 * Compile-time UI declarations with pure value expressions.
 * Only completed style calls and declaration macros produce reusable values.
 * Projections require their supported style input positions; ref variants require
 * computed variant keys or utility template substitutions.
 * Each argument expression must return an equivalent value for the same inputs and
 * must not mutate shared data or perform observable effects, including through
 * callees, getters, or value conversion. Calls need no PURE annotation. Functions
 * may mutate fresh local data used only to compute their result.
 *
 * Evaluation order and count are not API guarantees: unused expressions may be
 * omitted and class/value conversions may be combined. Do not rely on exceptions
 * from unused expressions. Read time/randomness and perform I/O, state updates,
 * Hooks, await/yield, imports, JSX, and class initialization outside the macro;
 * pass their resulting values. Reactive reads stay in the current computation
 * scope and must not be used only to establish a subscription.
 *
 * Names, utility definitions, animation options, and selector/query arguments are
 * static declarations. Dynamic identity selectors and view-transition keys obey
 * the same value-expression contract. Declaration-identity overloads are internal
 * checker metadata.
 */
export type StyleMacro<Props = object> = {
  <Id extends string>(
    ...inputs: [...StyleArgument[], StillwindDeclarationIdentity<Id>]
  ): StyleResult<Id, Props>;
  (...inputs: readonly StyleArgument[]): StyleResult<string, Props>;
  ref(): StyleRef;
  ref(label: string): StyleRef;
  /** The compiler requires the boundary to resolve to exactly one ref identity. */
  ref(boundary: StyleRef<string, string>): StyleRef;
  /** The compiler requires the boundary to resolve to exactly one ref identity. */
  ref(boundary: StyleRef<string, string>, label: string): StyleRef;
  ref<const Id extends string>(identity: StillwindDeclarationIdentity<Id>): StyleRef<Id>;
  ref<const Id extends string>(
    label: string,
    identity: StillwindDeclarationIdentity<Id>,
  ): StyleRef<Id>;
  ref<const BoundaryId extends string, const Id extends string>(
    boundary: StyleRef<BoundaryId, string>,
    identity: StillwindDeclarationIdentity<Id>,
  ): StyleRef<Id, BoundaryId>;
  ref<const BoundaryId extends string, const Id extends string>(
    boundary: StyleRef<BoundaryId, string>,
    label: string,
    identity: StillwindDeclarationIdentity<Id>,
  ): StyleRef<Id, BoundaryId>;
  keyframes(definition: StyleKeyframesDefinition): StyleKeyframes;
  keyframes<const Id extends string>(
    definition: StyleKeyframesDefinition,
    identity: StillwindDeclarationIdentity<Id>,
  ): StyleKeyframes<Id>;
  viewTransition(definition: StyleViewTransitionDefinition): StyleViewTransition;
  viewTransition<const Id extends string>(
    definition: StyleViewTransitionDefinition,
    identity: StillwindDeclarationIdentity<Id>,
  ): StyleViewTransition<Id>;
  /**
   * Adds anchor-scope without requiring a ref marker on the element.
   * Use this call directly in a style input. The argument follows the same literal
   * keyword or array rules as `ref.asAnchor`. An empty array emits no declaration.
   * This call does not attach any ref marker.
   */
  anchorScope(scope: AnchorScope): StyleRefProjection<string, string>;
};

/** Creates a macro entrypoint that reports missing compiler integration if executed. */
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
    },
  );
}
