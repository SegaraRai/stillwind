import { stillwindClassKey, stillwindMerge } from "stillwind/react/runtime";

import { selectedBoundary, selectedRef } from "./barrel";

export const attachedRef = stillwindMerge({[stillwindClassKey]:((selectedRef).c)});
export const anchorProvider = stillwindMerge({[stillwindClassKey]:(({"s_primary_8a6155b9":"_s_8655c0d0","s_secondary_1920670d":"_s_25b7b216"})[selectedRef.c])});
export const attachedBoundary = stillwindMerge({[stillwindClassKey]:((selectedBoundary).c)});
