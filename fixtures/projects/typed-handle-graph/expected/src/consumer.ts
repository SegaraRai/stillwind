import { stillwindClassKey, stillwindMerge } from "stillwind/react/runtime";

import { selectedRef, selectedScope } from "./barrel";

export const attachedRef = stillwindMerge({[stillwindClassKey]:((selectedRef).c)});
export const anchorProvider = stillwindMerge({[stillwindClassKey]:(({"s_primary_047d522c":"_s_69e145a4","s_secondary_994d70ad":"_s_0a26174f"})[selectedRef.anchor.c])});
export const attachedScope = stillwindMerge({[stillwindClassKey]:((selectedScope).c)});
