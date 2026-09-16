import { compactBoundary, primary, secondary, spaciousBoundary } from "./refs";

export const compact = Date.now() % 2 === 0;
export const selectedRef = compact ? primary : secondary;
export const selectedBoundary = compact ? compactBoundary : spaciousBoundary;
