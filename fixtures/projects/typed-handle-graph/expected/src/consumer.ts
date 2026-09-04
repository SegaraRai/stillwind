import { stillwindHandleClass } from "stillwind/react/runtime";
import { selectedRef, selectedScope } from "./barrel";

export const attachedRef = {className:stillwindHandleClass(selectedRef)};
export const anchorProvider = {className:stillwindHandleClass(selectedRef.anchor)};
export const attachedScope = {className:stillwindHandleClass(selectedScope)};
