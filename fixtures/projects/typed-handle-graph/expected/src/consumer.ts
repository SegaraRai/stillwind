import { stillwindClassKey, stillwindMerge } from "stillwind/react/runtime";

import { selectedRef, selectedScope } from "./barrel";

export const attachedRef = stillwindMerge({[stillwindClassKey]:((selectedRef).c)});
export const anchorProvider = stillwindMerge({[stillwindClassKey]:(({"s_047d522c":"_s_6dcedcf3","s_994d70ad":"_s_91806bde"})[selectedRef.anchor.c])});
export const attachedScope = stillwindMerge({[stillwindClassKey]:((selectedScope).c)});
