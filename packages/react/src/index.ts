/**
 * Compile-time React macro surface. Calls must be consumed by `@stillwind/vite`;
 * executing it at runtime means the compiler integration is missing.
 */
export type StyleAttributeName = `aria-${string}` | `data-${string}`

/** Augmented by the generated `.stillwind/stillwind.generated.d.ts` file. */
export interface StillwindUtilityVocabulary {}
export interface StillwindVariantVocabulary {}

type VocabularyUtility = keyof StillwindUtilityVocabulary
type UtilityAtom = [VocabularyUtility] extends [never] ? string : VocabularyUtility
type VocabularyVariant = keyof StillwindVariantVocabulary
type VariantAtom = [VocabularyVariant] extends [never] ? string : VocabularyVariant
/** One generated utility token, including one typed variant prefix. */
export type StillwindUtility = UtilityAtom | `${VariantAtom}:${UtilityAtom}`

// Utility lists are intentionally validated by the compiler. A recursive
// template-literal union for arbitrary-length class lists quickly exceeds
// TypeScript's union-complexity limit in real themes.
type UtilityString = string

export type StyleProps = {
  className?: string
} & Partial<Record<StyleAttributeName, unknown>>

export type StyleSelector = `&${string}`

declare const handleProjection: unique symbol

export type StyleHandleProjection = {
  readonly [handleProjection]: true
}

export type StyleHandle = {
  readonly anchor: StyleHandleProjection
  readonly position: StyleHandleProjection
  readonly scope: StyleHandleProjection
  container(type: "size" | "inline-size"): StyleHandleProjection
  at(query: string, utilities: string): StyleHandleProjection
  (strings: TemplateStringsArray): StyleHandleSelector
}

export type StyleHandleSelector = {
  readonly [handleProjection]: true
}

export type StyleAnimationUse = {
  readonly [handleProjection]: true
}

export type StyleAnimation = (options: string) => StyleAnimationUse

export type StyleAnimationDefinition = Readonly<Record<string, string>>

export type StyleViewTransitionUse = {
  readonly [handleProjection]: true
}

export type StyleViewTransition = StyleViewTransitionUse & {
  for(key: string | number): StyleViewTransitionUse
}

export type StyleViewTransitionDefinition = {
  group?: string
  old?: string
  new?: string
}

export type UnsafeStyle = {
  selector: `&${string}`
  css: Readonly<Record<string, string | number>>
  reason: string
}

export type StyleArgument =
  | UtilityString
  | false
  | null
  | undefined
  | readonly StyleArgument[]
  | Partial<Record<StyleAttributeName, unknown>>
  | Partial<Record<StyleSelector, UtilityString | StyleAnimationUse | false | null | undefined>>
  | StyleHandleProjection
  | StyleHandleSelector
  | StyleAnimationUse
  | StyleViewTransitionUse

export type StyleMacro = {
  (...inputs: readonly StyleArgument[]): StyleProps
  handle(): StyleHandle
  animation(definition: StyleAnimationDefinition): StyleAnimation
  viewTransition(definition: StyleViewTransitionDefinition): StyleViewTransition
  unsafe(input: UnsafeStyle): StyleProps
}

export const style = Object.assign(
  (..._arguments: readonly StyleArgument[]): StyleProps => {
    throw new Error(
      "stillwind/react style() is a compile-time macro. Add stillwind() to your Vite plugins.",
    )
  },
  {
    handle(): StyleHandle {
      throw new Error("stillwind/react style.handle() is a compile-time macro.")
    },
    animation(_definition: StyleAnimationDefinition): StyleAnimation {
      throw new Error("stillwind/react style.animation() is a compile-time macro.")
    },
    viewTransition(_definition: StyleViewTransitionDefinition): StyleViewTransition {
      throw new Error("stillwind/react style.viewTransition() is a compile-time macro.")
    },
    unsafe(_input: UnsafeStyle): StyleProps {
      throw new Error("stillwind/react style.unsafe() is a compile-time macro.")
    },
  },
) as StyleMacro
