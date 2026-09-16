import { compactSubtree, primary, secondary, spaciousSubtree } from "./refs";

export const compact = Date.now() % 2 === 0;
export const selectedRef = compact ? primary : secondary;
export const selectedSubtree = compact ? compactSubtree : spaciousSubtree;
