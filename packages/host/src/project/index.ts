/**
 * Owns Node analysis lifetimes and captured document revisions. AnalysisWorkspace opens configured
 * filesystem projects; AnalysisProject also accepts caller-supplied hosts and semantic backends.
 * EditorProject retains the last successful compilation for preview consumers.
 */
export { AnalysisProject, type AnalysisHost, type AnalysisResult } from "./analysis.ts";
export { EditorProject, type EditorAnalysis, type EditorDocument } from "./editor.ts";
export { createFileSystemAnalysisHost } from "./filesystem.ts";
export { compilerOptions, cssEntrypoint } from "./options.ts";
export { AnalysisWorkspace, type WorkspaceProject, type WorkspaceResult } from "./workspace.ts";
