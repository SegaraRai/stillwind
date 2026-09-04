import { style } from "stillwind/react";

// Non-ASCII text ensures Oxc byte offsets map back from TypeScript UTF-16 positions.
const _label = "🧭";
export const firstScope = style.scope("first");
export const first = style.ref(firstScope, "first");
export const secondScope = style.scope("second");
export const second = style.ref(secondScope, "second");
export const fadeIn = style.animation({ from: "opacity-0", to: "opacity-100" });
export const fadeOut = style.animation({ from: "opacity-100", to: "opacity-0" });
export const pageTitle = style.viewTransition({ old: "opacity-0", new: "opacity-100" });
export const pageCard = style.viewTransition({ old: "opacity-100", new: "opacity-0" });
