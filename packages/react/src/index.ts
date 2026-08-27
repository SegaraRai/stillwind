/**
 * Compile-time React macro surface. Calls must be consumed by `@stillwind/vite`;
 * executing it at runtime means the compiler integration is missing.
 */
export type StyleAttributeName = `aria-${string}` | `data-${string}`

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

export type StyleArgument =
  | string
  | false
  | null
  | undefined
  | readonly StyleArgument[]
  | Partial<Record<StyleAttributeName, unknown>>
  | Partial<Record<StyleSelector, string | StyleAnimationUse | false | null | undefined>>
  | StyleHandleProjection
  | StyleHandleSelector
  | StyleAnimationUse

export type StyleMacro = {
  (...inputs: readonly StyleArgument[]): StyleProps
  handle(): StyleHandle
  animation(definition: StyleAnimationDefinition): StyleAnimation
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
  },
) as StyleMacro
