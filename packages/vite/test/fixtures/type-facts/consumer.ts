import { style } from "stillwind/react";
import { selected, selectedAnimation, selectedTransition } from "./barrel";

export const attached = style(selected);
export const anchored = style(selected.anchor);
export const animated = style(selectedAnimation("infinite"));
export const transitioned = style(selectedTransition);
