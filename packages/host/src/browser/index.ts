export { defaultPluginExport, type PluginHost } from "../css/plugins/host.ts";
export type { PluginPathCompletion } from "../css/plugins/paths.ts";
export { TypeFactsError } from "../semantics/backend.ts";
export type {
  AvailableFacts,
  TypeFactsBackend,
  TypeFactSource,
  UnverifiedRange,
} from "../semantics/backend.ts";
export type { BrowserCheckerOptions } from "../semantics/browser/program.ts";
export { BrowserTypeFactsService } from "../semantics/browser/service.ts";
export {
  BrowserProject,
  virtualPath,
  type BrowserModuleAnalysis,
  type BrowserModuleCss,
  type BrowserProjectOptions,
  type BrowserStylesheetOutput,
} from "./project.ts";
export { resolveVirtualModule } from "./resolution.ts";
