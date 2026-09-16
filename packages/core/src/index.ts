/**
 * Compile-time React macro surface. Calls must be consumed by `@stillwind/vite`;
 * executing it at runtime means the compiler integration is missing.
 */
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

export type StyleProps = {
  className?: string;
  style?: Record<string, string>;
} & Partial<Record<StyleAttributeName, unknown>>;

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
declare const subtreeIdentity: unique symbol;
declare const refIdentity: unique symbol;
declare const animationIdentity: unique symbol;
declare const viewTransitionIdentity: unique symbol;
declare const styleIdentity: unique symbol;

export type StyleResult<Id extends string = string, Props = StyleProps> = Props & {
  readonly [styleIdentity]: Id;
};

export type StillwindDeclarationIdentity<Id extends string> = {
  readonly __stillwindDeclarationIdentity: Id;
};

export type StyleRefProjection<RefId extends string = string, SubtreeId extends string = never> = {
  readonly [refProjection]: true;
  readonly [refIdentity]: { readonly ref: RefId; readonly subtree: SubtreeId };
};

export type StyleSubtree<Id extends string = string> = {
  readonly [subtreeIdentity]: Id;
};

export type StyleRef<RefId extends string = string, SubtreeId extends string = never> = {
  readonly [refIdentity]: { readonly ref: RefId; readonly subtree: SubtreeId };
  readonly anchor: StyleRefProjection<RefId, SubtreeId>;
  readonly positionAnchor: StyleRefProjection<RefId, SubtreeId>;
  readonly anchorScope: StyleRefProjection<RefId, SubtreeId>;
  container(type: "size" | "inline-size"): StyleRefProjection<RefId, SubtreeId>;
  at(query: string): StyleRefVariant<RefId, SubtreeId>;
  at(strings: TemplateStringsArray): StyleRefVariant<RefId, SubtreeId>;
  (strings: TemplateStringsArray): StyleRefVariant<RefId, SubtreeId>;
  (): StyleRefVariant<RefId, SubtreeId>;
  (variants: string): StyleRefVariant<RefId, SubtreeId>;
};

export type StyleRefVariant<
  RefId extends string = string,
  SubtreeId extends string = never,
> = string & {
  readonly [refProjection]: true;
  readonly [refIdentity]: { readonly ref: RefId; readonly subtree: SubtreeId };
};

export type StyleAnimationUse<Id extends string = string> = {
  readonly [refProjection]: true;
  readonly [animationIdentity]: Id;
};

export type StyleAnimation<Id extends string = string> = {
  (options: string): StyleAnimationUse<Id>;
  readonly [animationIdentity]: Id;
};

export interface StyleAnimationDefinition {
  readonly [frameOrVariant: string]: string | StyleAnimationDefinition;
}

export type StyleViewTransitionUse<Id extends string = string> = {
  readonly [refProjection]: true;
  readonly [viewTransitionIdentity]: Id;
};

/** A declared view transition: participants share its class, and only `.for` names them. */
export type StyleViewTransition<Id extends string = string> = {
  readonly [refProjection]: true;
  readonly [viewTransitionIdentity]: Id;
  /**
   * Names the participants this use marks, under a pure value key.
   * A literal key folds into the stylesheet; an expression is read at runtime.
   */
  for(key?: string | number): StyleViewTransitionUse<Id>;
};

export type StyleViewTransitionDefinition = {
  group?: string;
  old?: string;
  new?: string;
  imagePair?: string;
};

/** Variant wrappers compose from outermost to innermost, just like utility prefixes. */
export type StyleVariantValue =
  | UtilityString
  | StyleAnimationUse
  | false
  | null
  | undefined
  | readonly StyleVariantValue[]
  | StyleVariantObject;

export interface StyleVariantObject {
  readonly [variant: StyleVariant]: StyleVariantValue;
  readonly [variant: StyleRefVariant]: StyleVariantValue;
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
  | StyleSubtree
  | StyleRef
  | StyleRefProjection
  | StyleRefVariant
  | StyleAnimationUse
  | StyleViewTransition
  | StyleViewTransitionUse;

/**
 * Compile-time UI declarations with pure value expressions.
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
export type StyleMacro<Props = StyleProps> = {
  <Id extends string>(
    ...inputs: [...StyleArgument[], StillwindDeclarationIdentity<Id>]
  ): StyleResult<Id, Props>;
  (...inputs: readonly StyleArgument[]): StyleResult<string, Props>;
  subtree(): StyleSubtree;
  subtree(name: string): StyleSubtree;
  subtree<const Id extends string>(identity: StillwindDeclarationIdentity<Id>): StyleSubtree<Id>;
  subtree<const Id extends string>(
    name: string,
    identity: StillwindDeclarationIdentity<Id>,
  ): StyleSubtree<Id>;
  ref(): StyleRef;
  ref(name: string): StyleRef;
  /** The compiler requires subtree to resolve to exactly one identity. */
  ref(subtree: StyleSubtree): StyleRef;
  /** The compiler requires subtree to resolve to exactly one identity. */
  ref(subtree: StyleSubtree, name: string): StyleRef;
  ref<const Id extends string>(identity: StillwindDeclarationIdentity<Id>): StyleRef<Id>;
  ref<const Id extends string>(
    name: string,
    identity: StillwindDeclarationIdentity<Id>,
  ): StyleRef<Id>;
  ref<const SubtreeId extends string, const Id extends string>(
    subtree: StyleSubtree<SubtreeId>,
    identity: StillwindDeclarationIdentity<Id>,
  ): StyleRef<Id, SubtreeId>;
  ref<const SubtreeId extends string, const Id extends string>(
    subtree: StyleSubtree<SubtreeId>,
    name: string,
    identity: StillwindDeclarationIdentity<Id>,
  ): StyleRef<Id, SubtreeId>;
  animation(definition: StyleAnimationDefinition): StyleAnimation;
  animation<const Id extends string>(
    definition: StyleAnimationDefinition,
    identity: StillwindDeclarationIdentity<Id>,
  ): StyleAnimation<Id>;
  viewTransition(definition: StyleViewTransitionDefinition): StyleViewTransition;
  viewTransition<const Id extends string>(
    definition: StyleViewTransitionDefinition,
    identity: StillwindDeclarationIdentity<Id>,
  ): StyleViewTransition<Id>;
};

export const style = Object.assign(
  (..._arguments: readonly StyleArgument[]): StyleProps => {
    throw new Error(
      "stillwind/react style() is a compile-time macro. Add stillwind() to your Vite plugins.",
    );
  },
  {
    subtree(_name?: string): StyleSubtree {
      throw new Error("stillwind/react style.subtree() is a compile-time macro.");
    },
    ref(_scopeOrName?: StyleSubtree | string, _name?: string): StyleRef {
      throw new Error("stillwind/react style.ref() is a compile-time macro.");
    },
    animation(_definition: StyleAnimationDefinition): StyleAnimation {
      throw new Error("stillwind/react style.animation() is a compile-time macro.");
    },
    viewTransition(_definition: StyleViewTransitionDefinition): StyleViewTransition {
      throw new Error("stillwind/react style.viewTransition() is a compile-time macro.");
    },
  },
) as StyleMacro;
