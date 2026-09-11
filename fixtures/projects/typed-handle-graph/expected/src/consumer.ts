import { stillwindClsx, stillwindHandleClass } from "stillwind/react/runtime";

import { selectedRef, selectedScope } from "./barrel";

export const attachedRef = {className:stillwindClsx(stillwindHandleClass(selectedRef))};
export const anchorProvider = {className:stillwindClsx(stillwindHandleClass(selectedRef.anchor,"_swp_s_primary_047d522c_anchor":"_s_69e145a4","_swp_s_secondary_994d70ad_anchor":"_s_0a26174f"))};
export const attachedScope = {className:stillwindClsx(stillwindHandleClass(selectedScope))};
