import type { StyleResult as Result } from "../macro/output.ts";
import {
  createStyleMacro,
  type StyleMacro as Macro,
  type StyleMacroWithoutForward as MacroWithoutForward,
} from "../macro/style.ts";

export type StyleProps = {
  class?: string;
  style?: Record<string, string>;
};

export type StyleResult<Id extends string = string> = Result<Id, StyleProps>;

export type StyleMacro = Macro<StyleProps>;

export type StyleMacroWithoutForward = MacroWithoutForward<StyleProps>;

export const style: StyleMacro = createStyleMacro<StyleProps>();
