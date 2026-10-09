/**
 * Compiles and analyzes the semantic dependencies demanded by one entry. The caller supplies
 * source operations, a native session, and a lazy semantic backend. Finalization and source-map
 * helpers adapt the generated JavaScript to a bundler's delivery paths and upstream transforms.
 */
export {
  analyzeModule,
  compileModule,
  warnRejectedNames,
  type CompilerHost,
  type ModuleAnalysis,
  type ModuleCompileOptions,
  type SemanticDependency,
} from "./compiler.ts";
export { compilerModuleId, fileModuleDescriptor } from "./filesystem.ts";
export { finalizeModuleWithMap, type FinalizedModuleEdit } from "./finalize.ts";
export { CompilationFailure } from "./module-graph.ts";
export { cleanModuleId, fileSourceKind, type ModuleDescriptor } from "./module-source.ts";
export type { PreparedSource } from "./protocol.ts";
export {
  composeCarrierSourceMap,
  composeFrameworkSourceMap,
  composeModuleSourceMap,
  moduleSourceMap,
  type CompilerMapping,
  type UpstreamSourceMap,
} from "./source-map.ts";
