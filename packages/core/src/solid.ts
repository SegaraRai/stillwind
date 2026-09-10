/** Solid's compile-time macro shares identities and vocabulary with the core API. */
import { style as macro } from "./index.ts";
import type { StyleAttributeName, StyleMacro as CoreStyleMacro } from "./index.ts";

export type * from "./index.ts";
export type StyleProps = { class?: string } & Partial<Record<StyleAttributeName, unknown>>;
export type StyleMacro = CoreStyleMacro<StyleProps>;
export const style: StyleMacro = macro as StyleMacro;
