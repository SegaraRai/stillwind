import type { StyleResult as Result } from "../macro/output.ts";
import { createStyleMacro } from "../macro/style.ts";
import type { StyleMacro as Macro } from "../macro/style.ts";

export type StyleProps = {
  class?: string;
  style?: Record<string, string>;
};

export type StyleResult<Id extends string = string> = Result<Id, StyleProps>;

export type StyleMacro = Macro<StyleProps>;

export const style: StyleMacro = createStyleMacro<StyleProps>();
