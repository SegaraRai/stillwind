import { style } from "stillwind/react";

import { selectedBoundary, selectedRef } from "./barrel";

export const attachedRef = style(selectedRef);
export const anchorProvider = style(selectedRef.anchor);
export const attachedBoundary = style(selectedBoundary);
