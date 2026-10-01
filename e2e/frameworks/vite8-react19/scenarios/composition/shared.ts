import { style } from "stillwind/react";

import { base } from "./base";

export const middle = style(base, "m-4");
export const back = style(middle, "m-10");
export const left = style(base, "m-6");
export const right = style(base, "m-8");
export const diamond = style(left, right, "m-2");
