import { style } from "stillwind/react";

// Non-ASCII text ensures Oxc byte offsets map back from TypeScript UTF-16 positions.
const _label = "🧭";
export const firstSubtree = style.subtree("first");
export const first = style.ref(firstSubtree, "first");
export const secondSubtree = style.subtree("second");
export const second = style.ref(secondSubtree, "second");
export const fadeIn = style.animation({
  from: "opacity-0 bg-[url(./support.svg)]",
  to: "opacity-100",
});
export const fadeOut = style.animation({ from: "opacity-100", to: "opacity-0" });
export const pageTitle = style.viewTransition({ old: "opacity-0", new: "opacity-100" });
export const pageCard = style.viewTransition({ old: "opacity-100", new: "opacity-0" });
export const unusedStyle = style("flex");
export const unusedAnimation = style.animation({ from: "opacity-50", to: "opacity-75" });
