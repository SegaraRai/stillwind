/**
 * URI-based analysis contracts used by language servers and compiler workers.
 * Node adapters are available from `@stillwind/analysis/node`. `BrowserAnalysisWorkspace` in
 * the browser subpath compiles captured virtual files. Embeddings with their own compiler
 * services can use `createMemoryAnalysisWorkspace` in the memory subpath.
 */

export type { EditorRequest, EditorResult } from "@stillwind/host";

export { lintReports } from "./reports.ts";
export type {
  AnalysisDependency,
  AnalysisResult,
  AnalysisWorkspaceContext,
  AnalysisWorkspaceService,
  OpenAnalysisWorkspaces,
  OpenWorkspaces,
  WorkspaceEditorResult,
  WorkspaceResult,
} from "./workspace.ts";
