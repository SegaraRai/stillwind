/**
 * Compile-time React macro surface. Calls must be consumed by `@stillwind/vite`;
 * executing it at runtime means the compiler integration is missing.
 */
export type StyleAttributeName = `aria-${string}` | `data-${string}`

export type StyleProps = {
  className?: string
} & Partial<Record<StyleAttributeName, unknown>>

export type StyleArgument =
  | string
  | false
  | null
  | undefined
  | Partial<Record<StyleAttributeName, unknown>>

export function style(..._arguments: readonly StyleArgument[]): StyleProps {
  throw new Error(
    "stillwind/react style() is a compile-time macro. Add stillwind() to your Vite plugins.",
  )
}
