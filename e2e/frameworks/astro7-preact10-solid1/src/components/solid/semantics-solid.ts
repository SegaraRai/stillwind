import { style } from "stillwind/solid";

import { probeBase } from "./semantics-solid-base";

export const probeMiddle = style(probeBase, "m-[16px]");
export const probeComposed = style(probeMiddle, "m-[12px]");
export const probeLeft = style(probeBase, "text-[#123456]");
export const probeRight = style(probeBase, "bg-[#abcdef]");
export const probeDiamond = style(probeLeft, probeRight, "m-[2px]");
