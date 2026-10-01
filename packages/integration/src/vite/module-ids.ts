import { cleanModuleId, compilerModuleId } from "@stillwind/host";

export const virtualCssId = "virtual:stillwind.css";
export const runtimeId = "\0stillwind:runtime";
export const valuePrefix = "\0stillwind:value:";
export const fragmentPrefix = "\0stillwind:fragment:";
export const projectionPrefix = "\0stillwind:projection:";
export const producerPrefix = "\0stillwind:producer:";
export const runtimeSpecifier = "virtual:stillwind/runtime";

/** Returns a root-relative compiler identity while keeping a module's query suffix. */
export function sourceModuleId(root: string, id: string): string {
  const file = cleanModuleId(id);
  return compilerModuleId(root, file) + id.slice(file.length);
}
