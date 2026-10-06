import path from "node:path";

import { findConfiguration } from "@stillwind/config/node";
import type { WorkspaceResult } from "@stillwind/host";

import { AnalysisWorker } from "./worker-client.ts";

/** Lint results preserve project membership and identify incomplete analysis. */
export interface LintResult {
  /** Absolute path of the evaluated configuration. */
  config: string;
  /** Target results in configured project order, including overlapping project memberships. */
  results: WorkspaceResult[];
  /**
   * - 0: successful checks
   * - 1: compiler errors or denied lint
   * - 2: incomplete execution
   */
  exitCode: 0 | 1 | 2;
}

/**
 * Analyzes configured target scopes in a child process and closes it before returning.
 *
 * @param options - Configuration discovery and optional target filters. Omission checks all
 * configured targets from the process working directory.
 * @returns Results and an exit code. Empty target selection or incomplete analysis returns code 2.
 * @throws If configuration discovery, worker initialization, target enumeration, or cleanup fails.
 */
export async function lint(
  options: {
    /** Explicit configuration path, absolute or relative to cwd. Omission searches upward from cwd. */
    config?: string;
    /** Directory for configuration discovery and relative config paths. Defaults to process.cwd(). */
    cwd?: string;
    /** Additional globs relative to each project root. Omission or an empty list retains all configured targets. */
    patterns?: string[];
  } = {},
): Promise<LintResult> {
  const cwd = options.cwd ?? process.cwd();
  const config = options.config ? path.resolve(cwd, options.config) : findConfiguration(cwd);
  if (!config) {
    throw new Error(`Cannot find stillwind.config.ts above ${cwd}`);
  }
  const worker = new AnalysisWorker();
  try {
    await worker.open(config);
    const results = await worker.lint(options.patterns);
    const exitCode =
      !results.length || results.some(({ result }) => result.status !== "complete")
        ? 2
        : results.some(({ result }) => result.reports.some((report) => report.severity === "error"))
          ? 1
          : 0;
    return { config, results, exitCode };
  } finally {
    await worker.close();
  }
}

/**
 * Formats findings and incomplete analysis for terminal output.
 *
 * @param result - Lint results whose reports use original document locations.
 * @returns Text with one-based line and column positions. Duplicate findings are removed per
 * project. Findings from overlapping projects remain separate. Successful results can return empty text.
 */
export function formatLint(result: LintResult): string {
  const lines: string[] = [];
  const seen = new Set<string>();
  for (const entry of result.results) {
    for (const report of entry.result.reports) {
      const location = report.primary.location;
      const key = JSON.stringify([entry.project, report.code, location, report.message]);
      if (seen.has(key)) {
        continue;
      }
      seen.add(key);
      const at =
        report.primary.scope === "document"
          ? location.uri
          : `${location.uri}:${location.startLine + 1}:${location.startColumn + 1}`;
      lines.push(
        `[project ${entry.project + 1}] ${at}: ${report.severity} ${report.code}: ${report.message}`,
      );
      for (const related of report.related) {
        lines.push(
          `  ${related.location.uri}:${related.location.startLine + 1}:${related.location.startColumn + 1}${related.message ? `: ${related.message}` : ""}`,
        );
      }
      lines.push(...report.notes.map((note) => `  ${note}`));
    }
    if (entry.result.status !== "complete") {
      lines.push(
        `[project ${entry.project + 1}] ${entry.result.entry}: incomplete analysis${entry.result.error ? `: ${entry.result.error}` : ""}`,
      );
      for (const dependency of entry.result.graph?.dependencies ?? []) {
        if (dependency.status === "unresolved" || dependency.status === "unsupported") {
          lines.push(`  ${dependency.status}: ${dependency.specifier} from ${dependency.importer}`);
        }
      }
      for (const range of entry.result.graph?.unverified ?? []) {
        lines.push(
          `  unverified: ${range.moduleId}${range.start === undefined ? "" : `:${range.start}-${range.end}`}: ${range.reason}`,
        );
      }
    }
  }
  if (!result.results.length) {
    lines.push("No analysis targets matched the configured scopes");
  }
  return lines.join("\n");
}
