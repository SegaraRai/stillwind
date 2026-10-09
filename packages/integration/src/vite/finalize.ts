import { finalizeModuleWithMap as finalizeHostModuleWithMap } from "@stillwind/host";

import { virtualCssId } from "./module-ids.ts";

/**
 * Prepends the shared virtual stylesheet import when automatic delivery is requested.
 * @param code - Compiled JavaScript before host finalization.
 * @param includeStylesheet - Whether to activate the shared stylesheet. Defaults to true.
 * @returns Final JavaScript text.
 */
export function finalizeModule(code: string, includeStylesheet = true) {
  return finalizeModuleWithMap(code, includeStylesheet).code;
}

/**
 * Finalizes source while retaining prefix metadata for source-map generation.
 * @param code - Compiled JavaScript before finalization.
 * @param includeStylesheet - Whether to prepend the shared virtual stylesheet import.
 * @returns Final code, the inserted prefix, and host finalization's removed intervals.
 */
export function finalizeModuleWithMap(code: string, includeStylesheet = true) {
  return finalizeHostModuleWithMap(code, includeStylesheet ? virtualCssId : undefined);
}
