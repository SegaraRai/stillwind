import {
  style as classObjectStyle,
  type StyleMacroWithoutForward,
} from "../outputs/class-object.ts";

export { applyStyle, clearStyle } from "../dom/style.ts";
export type * from "../macro/index.ts";
export type { StyleResult as StyleResultOf } from "../macro/output.ts";
export type {
  StyleMacroWithoutForward as StyleMacro,
  StyleProps,
  StyleResult,
} from "../outputs/class-object.ts";

/** The `style` macro. Elements here receive no component props, so it has no `forward()`. */
export const style: StyleMacroWithoutForward = classObjectStyle;
