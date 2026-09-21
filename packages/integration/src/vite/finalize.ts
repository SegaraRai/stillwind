import { finalizeModuleWithMap as finalizeHostModuleWithMap } from "@stillwind/host";

import { virtualCssId } from "./module-ids.ts";

export function finalizeModule(code: string, includeStylesheet = true) {
  return finalizeModuleWithMap(code, includeStylesheet).code;
}

export function finalizeModuleWithMap(code: string, includeStylesheet = true) {
  return finalizeHostModuleWithMap(code, includeStylesheet ? virtualCssId : undefined);
}
