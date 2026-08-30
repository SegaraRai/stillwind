/**
 * Compile-time React macro surface. Calls must be consumed by `@stillwind/vite`;
 * executing it at runtime means the compiler integration is missing.
 */
export type StyleAttributeName = `aria-${string}` | `data-${string}`;

/** Augmented by the generated `.stillwind/stillwind.generated.d.ts` file. */
export interface StillwindUtilityVocabulary {}
export interface StillwindVariantVocabulary {}

type VocabularyUtility = Extract<keyof StillwindUtilityVocabulary, string>;
type VocabularyVariant = Extract<keyof StillwindVariantVocabulary, string>;
type UtilityWithVariant<Utility extends string, Variant extends string> = [Utility] extends [never]
  ? string
  : [Variant] extends [never]
    ? Utility
    : Utility | `${Variant}:${Utility}`;
/** One generated utility token, including one typed variant prefix. */
export type StillwindUtility = UtilityWithVariant<VocabularyUtility, VocabularyVariant>;

// Utility lists are intentionally validated by the compiler. A recursive
// template-literal union for arbitrary-length class lists quickly exceeds
// TypeScript's union-complexity limit in real themes.
type UtilityString = string;

export type StyleProps = {
  className?: string;
} & Partial<Record<StyleAttributeName, unknown>>;

export type StyleSelector = `&${string}`;

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
declare const scopeIdentity: unique symbol;
declare const refIdentity: unique symbol;

export type StyleRefProjection = {
  readonly [refProjection]: true;
};

export type StyleScope = {
  readonly [scopeIdentity]: true;
};

export type StyleRef = {
  readonly [refIdentity]: true;
  readonly anchor: StyleRefProjection;
  readonly position: StyleRefProjection;
  readonly anchorScope: StyleRefProjection;
  container(type: "size" | "inline-size"): StyleRefProjection;
  at(query: string, utilities: string): StyleRefProjection;
  (strings: TemplateStringsArray): StyleRefSelector;
};

export type StyleRefSelector = {
  readonly [refProjection]: true;
};

export type StyleAnimationUse = {
  readonly [refProjection]: true;
};

export type StyleAnimation = (options: string) => StyleAnimationUse;

export type StyleAnimationDefinition = Readonly<Record<string, string>>;

export type StyleViewTransitionUse = {
  readonly [refProjection]: true;
};

export type StyleViewTransition = StyleViewTransitionUse & {
  for(key: string | number): StyleViewTransitionUse;
};

export type StyleViewTransitionDefinition = {
  group?: string;
  old?: string;
  new?: string;
};

type StyleObject = Partial<Record<StyleAttributeName, unknown>> &
  Partial<Record<StyleSelector, UtilityString | StyleAnimationUse | false | null | undefined>> & {
    readonly class?: StillwindClassValue;
  };

export type StyleArgument =
  | UtilityString
  | false
  | null
  | undefined
  | readonly StyleArgument[]
  | StyleObject
  | StyleScope
  | StyleRef
  | StyleRefProjection
  | StyleRefSelector
  | StyleAnimationUse
  | StyleViewTransitionUse;

export type StyleMacro = {
  (...inputs: readonly StyleArgument[]): StyleProps;
  scope(): StyleScope;
  scope(name: string): StyleScope;
  ref(): StyleRef;
  ref(name: string): StyleRef;
  ref(scope: StyleScope): StyleRef;
  ref(scope: StyleScope, name: string): StyleRef;
  animation(definition: StyleAnimationDefinition): StyleAnimation;
  viewTransition(definition: StyleViewTransitionDefinition): StyleViewTransition;
};

export const style = Object.assign(
  (..._arguments: readonly StyleArgument[]): StyleProps => {
    throw new Error(
      "stillwind/react style() is a compile-time macro. Add stillwind() to your Vite plugins.",
    );
  },
  {
    scope(_name?: string): StyleScope {
      throw new Error("stillwind/react style.scope() is a compile-time macro.");
    },
    ref(_scopeOrName?: StyleScope | string, _name?: string): StyleRef {
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
