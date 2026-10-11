/**
 * Provides filesystem linting, sorting, and URI adapters backed by isolated Node workers.
 * SourceFormatter sorts unsaved buffers without writing. format and lint with fix save planned changes.
 * openNodeWorkspaces supplies services owned and closed by the embedding language server.
 */

export { format, type FormatResult } from "./format.ts";
export { formatLint, lint, lintReports, type LintResult } from "./lint.ts";
export { openNodeWorkspaces } from "./node-workspace.ts";
export type { RewriteOptions } from "./rewrite-files.ts";
export type { RewriteInput, RewriteResult } from "./rewrite.ts";
export { findFormatterConfig, SourceFormatter } from "./source-formatter.ts";
