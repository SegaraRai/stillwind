import { style } from "stillwind/react";

import { selectedRef, selectedScope } from "./barrel";

export const attachedRef = style(selectedRef);
export const anchorProvider = style(selectedRef.anchor);
export const attachedScope = style(selectedScope);
