import { stillwindHandleClass } from "stillwind/react/runtime";

import { selectedRef, selectedScope } from "./barrel";

export const attachedRef = {className:stillwindHandleClass(selectedRef)};
export const anchorProvider = {className:stillwindHandleClass(selectedRef.anchor,{"_swp_s_primary_047d522c_anchor":"_swp_s_primary_047d522c_anchor _s_16df9693","_swp_s_secondary_994d70ad_anchor":"_swp_s_secondary_994d70ad_anchor _s_2fa82b1f"})};
export const attachedScope = {className:stillwindHandleClass(selectedScope)};
