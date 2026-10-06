import path from "node:path";

import { findConfiguration, type WorkspaceResult } from "@stillwind/host";

import { AnalysisWorker } from "./worker-client.ts";

/** Lint results preserve project membership and identify incomplete analysis. */
export interface LintResult {
  config: string;
  results: WorkspaceResult[];
  /** 0 for successful checks, 1 for compiler errors or denied lint, 2 for incomplete execution. */
  exitCode: 0 | 1 | 2;
}

/** Runs the configured scopes in an isolated analysis process and closes all hosts afterward. */
export async function lint(
  options: { config?: string; cwd?: string; patterns?: string[] } = {},
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

/** Renders each finding once per project, including failures without a source range. */
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
