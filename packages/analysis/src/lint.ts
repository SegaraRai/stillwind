import type { WorkspaceResult } from "@stillwind/host";

import { lintReports } from "./index.ts";
import { resolveRewriteConfig, rewriteFiles, type RewriteOptions } from "./rewrite-files.ts";
import { AnalysisWorker } from "./worker-client.ts";

/** Lint results preserve project membership and identify incomplete analysis. */
export interface LintResult {
  /** Absolute path of the evaluated configuration. */
  config: string;
  /** Target results in configured project order, including overlapping project memberships. */
  results: WorkspaceResult[];
  /** Files whose bytes changed when fix was requested. */
  changed?: string[];
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
 * @returns Results and an exit code. Readonly lint returns code 2 for empty selection or incomplete analysis.
 * @throws If configuration discovery, worker initialization, target enumeration, or cleanup fails.
 * Fix mode also throws when planning or saving fails, including empty selection and incomplete analysis.
 */
export async function lint(
  options: RewriteOptions & {
    /** Applies converged fixes and reports findings from the final source buffers. */
    fix?: boolean;
  } = {},
): Promise<LintResult> {
  if (options.fix) {
    const result = await rewriteFiles(options, "fixes", true);
    return {
      ...result,
      exitCode: result.results.some(({ result }) =>
        result.reports.some((report) => report.severity === "error"),
      )
        ? 1
        : 0,
    };
  }
  const config = resolveRewriteConfig(options);
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
  for (const { project, report } of lintReports(result.results)) {
    const location = report.primary.location;
    const at =
      report.primary.scope === "document"
        ? location.uri
        : `${location.uri}:${location.startLine + 1}:${location.startColumn + 1}`;
    lines.push(
      `[project ${project + 1}] ${at}: ${report.severity} ${report.code}: ${report.message}`,
    );
    for (const related of report.related) {
      lines.push(
        `  ${related.location.uri}:${related.location.startLine + 1}:${related.location.startColumn + 1}${related.message ? `: ${related.message}` : ""}`,
      );
    }
    lines.push(...report.notes.map((note) => `  ${note}`));
  }
  for (const entry of result.results) {
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
