import { compactScope, primary, secondary, spaciousScope } from "./refs";

export const compact = Date.now() % 2 === 0;
export const selectedRef = compact ? primary : secondary;
export const selectedScope = compact ? compactScope : spaciousScope;
