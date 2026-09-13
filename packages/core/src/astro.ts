/** Astro's compile-time macro is consumed before server/client script extraction. */

import { style as macro } from "./index.ts";
import type { StyleAttributeName, StyleMacro as CoreStyleMacro } from "./index.ts";

export * from "./class-props.ts";

export type StyleProps = { class?: string; style?: string } & Partial<
  Record<StyleAttributeName, unknown>
>;
export type StyleMacro = CoreStyleMacro<StyleProps>;
export const style: StyleMacro = macro as unknown as StyleMacro;
