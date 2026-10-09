import { cleanModuleId, compilerModuleId } from "@stillwind/host";

/** Shared stylesheet alias resolved to the environment's development runtime or production carrier. */
export const virtualCssId = "virtual:stillwind.css";
/** Resolved runtime identity recognized before registry-backed modules. */
export const runtimeId = "\0stillwind:runtime";
/** Resolved prefix for explicit module CSS values issued by ModuleRegistry. */
export const valuePrefix = "\0stillwind:value:";
/** Resolved prefix for stylesheet fragment references issued by ModuleRegistry. */
export const fragmentPrefix = "\0stillwind:fragment:";
/** Resolved prefix for CSS-only source projection references. */
export const projectionPrefix = "\0stillwind:projection:";
/** Resolved prefix for producer registration modules. */
export const producerPrefix = "\0stillwind:producer:";
/** Public alias used by generated producer modules to import the environment runtime. */
export const runtimeSpecifier = "virtual:stillwind/runtime";

/**
 * Derives a compiler identity while retaining the source identity's query or fragment suffix.
 * @param root - Absolute project root used by compilerModuleId.
 * @param id - Full source resolver identity.
 * @returns The compiler identity of the cleaned path followed by its original suffix.
 */
export function sourceModuleId(root: string, id: string): string {
  const file = cleanModuleId(id);
  return compilerModuleId(root, file) + id.slice(file.length);
}
