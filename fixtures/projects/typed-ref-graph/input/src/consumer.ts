import { style } from "stillwind/react";

import { selectedRef, selectedScope } from "./barrel";

export const attachedRef = style(selectedRef);

export const anchorProvider = style(selectedRef, selectedRef.asAnchor([selectedRef]));

export const attachedScope = style(selectedScope);

export const scopedSelector = style({
  [selectedRef("hover").in(selectedScope)]: "flex",
});

export const anchorScope = style(style.anchorScope([selectedRef]));
