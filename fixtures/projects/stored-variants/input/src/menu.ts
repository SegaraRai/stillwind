import { style } from "stillwind/react";

import { hovered, pressed, wideAndHovered } from "./barrel";
import { menu, trigger } from "./conditions";

export const root = style(menu, menu.asContainer("inline-size"), "relative");

export const button = style(trigger, "px-2");

export const icon = style({
  [hovered]: "rotate-90",
  [pressed]: "scale-95",
});

export const label = style(`${hovered}:underline ${wideAndHovered}:font-bold`);

export const caret = style(`${hovered}:${pressed}:opacity-50`);
