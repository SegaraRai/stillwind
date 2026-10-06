/**
 * Supplies declaration identities and module resolution for prepared sources. TypeFactsBackend
 * defines the contract for custom checkers; TypeScriptFactsService implements it for Node using
 * a reusable checker selected by resolveTypeFactsEnvironment.
 */
export { TypeFactsError } from "./backend.ts";
export type {
  AvailableFacts,
  CheckerSpan,
  SemanticInputs,
  TypeFactsBackend,
  TypeFactSource,
  UnverifiedRange,
} from "./backend.ts";
export {
  resolveTypeFactsEnvironment,
  TypeScriptFactsService,
  type TypeFact,
  type TypeFactsEnvironment,
  type TypeFactsInputs,
  type TypeScriptFactsStats,
  type TypeScriptProject,
} from "./type-facts.ts";
