import { compactScope, primary, secondary, spaciousScope } from "./handles";

export const compact = Date.now() % 2 === 0;
export const selectedRef = compact ? primary : secondary;
export const selectedScope = compact ? compactScope : spaciousScope;
