import { createStyleMacro } from "../macro/index.ts";
import type {
  StyleAttributeName,
  StyleMacro as Macro,
  StyleResult as Result,
} from "../macro/index.ts";

export type * from "../macro/index.ts";

export type StyleProps = {
  class?: string;
  style?: string;
} & Partial<Record<StyleAttributeName, unknown>>;
export type StyleResult<Id extends string = string> = Result<Id, StyleProps>;
export type StyleMacro = Macro<StyleProps>;
export const style: StyleMacro = createStyleMacro<StyleProps>();
