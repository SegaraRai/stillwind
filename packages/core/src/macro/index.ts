/**
 * Internal contracts and construction of the framework-specific `style` macros.
 *
 * Import `style` from a framework entrypoint such as `stillwind/react`.
 * Declaration values and completed style props can be stored and passed between
 * modules. Projection calls and ref variants must be consumed directly inside
 * `style()` at the positions described by their methods.
 *
 * @packageDocumentation
 * @internal
 */

export type * from "./css.ts";
export type * from "./cva.ts";
export type {
  StyleArgument,
  StyleCustomProperties,
  StyleVariantObject,
  StyleVariantValue,
} from "./input.ts";
export type {
  StyleKeyframes,
  StyleKeyframesDefinition,
  StyleKeyframesFrames,
  StyleKeyframesUse,
} from "./keyframes.ts";
export type { StyleResult } from "./output.ts";
export type { StyleRefProjection } from "./projection.ts";
export type {
  AnchorScope,
  ContainerType,
  ContainerVariant,
  CssWideKeyword,
  RefScope,
  RefVariant,
  ScopedRefVariant,
  StyleRef,
} from "./refs.ts";
export { createStyleMacro, type StillwindDeclarationIdentity, type StyleMacro } from "./style.ts";
export type { StillwindClassValue, StyleCustomPropertyValue, StyleVariant } from "./values.ts";
export type {
  StyleViewTransition,
  StyleViewTransitionDefinition,
  StyleViewTransitionUse,
} from "./view-transition.ts";
