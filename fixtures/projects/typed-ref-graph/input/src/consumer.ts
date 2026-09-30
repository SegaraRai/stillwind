import { style } from "stillwind/react";

import { selectedBoundary, selectedRef } from "./barrel";

export const attachedRef = style(selectedRef);
export const anchorProvider = style(selectedRef, selectedRef.asAnchor([selectedRef]));
export const attachedBoundary = style(selectedBoundary);

export const anchorScope = style(style.anchorScope([selectedRef]));
