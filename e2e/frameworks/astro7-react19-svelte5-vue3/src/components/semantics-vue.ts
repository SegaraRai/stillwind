import { style } from "stillwind/vue";

import { probeBase } from "./semantics-vue-base";

export const probeMiddle = style(probeBase, "m-[16px]");
export const probeComposed = style(probeMiddle, "m-[12px]");
export const probeLeft = style(probeBase, "text-[#123456]");
export const probeRight = style(probeBase, "bg-[#abcdef]");
export const probeDiamond = style(probeLeft, probeRight, "m-[2px]");
