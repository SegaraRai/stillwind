/**
 * Supplies declaration identities and module resolution for prepared sources. TypeFactsBackend
 * defines the contract for custom checkers; TypeScriptFactsService implements it for Node using
 * a reusable checker selected by resolveTypeFactsEnvironment.
 */

export {
  TypeFactsError,
  type AvailableFacts,
  type CheckerSpan,
  type SemanticInputs,
  type TypeFactsBackend,
  type TypeFactSource,
  type UnverifiedRange,
} from "./backend.ts";
export type { TypeScriptFactsStats } from "./node/checker.ts";
export {
  resolveTypeFactsEnvironment,
  type TypeFactsEnvironment,
  type TypeFactsInputs,
  type TypeScriptProject,
} from "./node/project.ts";
export { TypeScriptFactsService, type TypeFact } from "./node/service.ts";
