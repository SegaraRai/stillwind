import { style } from "stillwind/react";

import { selectedRef, selectedSubtree } from "./barrel";

export const attachedRef = style(selectedRef);
export const anchorProvider = style(selectedRef.anchor);
export const attachedSubtree = style(selectedSubtree);
