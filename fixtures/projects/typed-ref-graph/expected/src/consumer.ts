import { stillwindClassKey, stillwindMerge } from "stillwind/react/runtime";

import { selectedRef, selectedScope } from "./barrel";

export const attachedRef = stillwindMerge({[stillwindClassKey]:((selectedRef).c)});
export const anchorProvider = stillwindMerge({[stillwindClassKey]:(({"s_8a6155b9":"_s_0fcfb31f","s_1920670d":"_s_bfee00d7"})[selectedRef.anchor.c])});
export const attachedScope = stillwindMerge({[stillwindClassKey]:((selectedScope).c)});
