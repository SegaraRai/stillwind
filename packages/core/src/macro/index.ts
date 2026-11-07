/**
 * Internal contracts and construction of the framework-specific `style` macros.
 *
 * Import `style` from a framework entrypoint such as `stillwind/react`.
 * Every macro call is a value. Completed style props, declarations, and the
 * results of `style.media()` and `style.css()` exist at runtime and can be stored and
 * passed between modules. Stored variants from `style.when()` exist only at compile
 * time: they are `undefined` at runtime, but `style()` reads them through bindings.
 * Projection calls and ref variants must be consumed directly inside `style()` at the
 * positions described by their methods.
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
  StyleVariantKey,
} from "./refs.ts";
export { createStyleMacro, type StillwindDeclarationIdentity, type StyleMacro } from "./style.ts";
export type { StillwindClassValue, StyleCustomPropertyValue, StyleVariant } from "./values.ts";
export type { StyleStoredVariant } from "./when.ts";
export type {
  StyleViewTransition,
  StyleViewTransitionDefinition,
  StyleViewTransitionUse,
} from "./view-transition.ts";
