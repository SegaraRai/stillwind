/** Shared type surface for frameworks whose spread attribute is named `class`. */

import { style as macro } from "./index.ts";
import type { StyleAttributeName, StyleMacro as CoreStyleMacro } from "./index.ts";

export type * from "./index.ts";

export type StyleProps = {
  class?: string;
  style?: Record<string, string>;
} & Partial<Record<StyleAttributeName, unknown>>;
export type StyleMacro = CoreStyleMacro<StyleProps>;
export const style: StyleMacro = macro as StyleMacro;
