import { stillwindClassKey, stillwindMerge } from "stillwind/react/runtime";

import { selectedRef, selectedSubtree } from "./barrel";

export const attachedRef = stillwindMerge({[stillwindClassKey]:((selectedRef).c)});
export const anchorProvider = stillwindMerge({[stillwindClassKey]:(({"s_primary_8a6155b9":"_s_9d2f1a87","s_secondary_1920670d":"_s_aba6b76e"})[selectedRef.c])});
export const attachedSubtree = stillwindMerge({[stillwindClassKey]:((selectedSubtree).c)});
