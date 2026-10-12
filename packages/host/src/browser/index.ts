export { TypeFactsError } from "../semantics/backend.ts";
export type {
  AvailableFacts,
  TypeFactsBackend,
  TypeFactSource,
  UnverifiedRange,
} from "../semantics/backend.ts";
export { BrowserTypeFactsService, type BrowserCheckerOptions } from "../semantics/browser.ts";
export {
  BrowserProject,
  virtualPath,
  type BrowserModuleAnalysis,
  type BrowserModuleCss,
  type BrowserProjectOptions,
  type BrowserStylesheetOutput,
} from "./project.ts";
export { resolveVirtualModule } from "./resolution.ts";
