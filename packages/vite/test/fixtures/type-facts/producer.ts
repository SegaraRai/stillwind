import { style } from "stillwind/react";

// Non-ASCII text ensures Oxc byte offsets map back from TypeScript UTF-16 positions.
const _label = "🧭";
export const firstScope = style.scope("first");
export const first = style.ref(firstScope, "first");
export const secondScope = style.scope("second");
export const second = style.ref(secondScope, "second");
