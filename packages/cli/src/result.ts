import { lintReports, type FormatResult, type LintResult } from "@stillwind/analysis/node";

export type CliCommand = "lint" | "fmt" | null;
export type CliDiagnostic = ReturnType<typeof lintReports>[number];

export interface CliFailure {
  project?: number;
  file?: string;
  message: string;
  details: { file: string; message: string }[];
}

export interface CliResult {
  command: CliCommand;
  config: string | null;
  diagnostics: CliDiagnostic[];
  failures: CliFailure[];
  changed: string[];
  summary: {
    files: number;
    errors: number;
    warnings: number;
    failures: number;
    changed: number;
    suggestions: number;
    warningLimitExceeded: boolean;
  };
  exitCode: 0 | 1 | 2;
}

export interface ResultPolicy {
  check?: boolean;
  quiet?: boolean;
  maxWarnings?: number;
}

/** Projects analysis into CLI data before visibility filtering and rendering. */
export function cliResult(
  command: CliCommand,
  result: Pick<LintResult | FormatResult, "config" | "results" | "changed">,
  policy: ResultPolicy = {},
): CliResult {
  const diagnostics = command === "fmt" ? [] : lintReports(result.results);
  const failures: CliFailure[] = result.results.flatMap(({ project, result }) =>
    result.status === "complete"
      ? []
      : [
          {
            project,
            file: result.entry,
            message: `Incomplete analysis${result.error ? `: ${result.error}` : ""}`,
            details: [
              ...(result.graph?.dependencies ?? []).flatMap((dependency) =>
                dependency.status === "unresolved" || dependency.status === "unsupported"
                  ? [
                      {
                        file: dependency.importer,
                        message: `${dependency.status}: ${dependency.specifier}`,
                      },
                    ]
                  : [],
              ),
              ...(result.graph?.unverified ?? []).map((range) => ({
                file: range.moduleId,
                message: `unverified${range.start === undefined ? "" : ` bytes ${range.start}-${range.end}`}: ${range.reason}`,
              })),
            ],
          },
        ],
  );
  if (!result.results.length) {
    failures.push({ message: "No analysis targets matched the configured scopes", details: [] });
  }
  const changed = [...new Set(result.changed ?? [])].sort();
  const errors = diagnostics.filter(({ report }) => report.severity === "error").length;
  const warnings = diagnostics.length - errors;
  const warningLimitExceeded = policy.maxWarnings !== undefined && warnings > policy.maxWarnings;
  return {
    command,
    config: result.config,
    diagnostics: policy.quiet
      ? diagnostics.filter(({ report }) => report.severity === "error")
      : diagnostics,
    failures,
    changed,
    summary: {
      files: new Set(result.results.map(({ result }) => result.entry)).size,
      errors,
      warnings,
      failures: failures.length,
      changed: changed.length,
      suggestions: diagnostics.filter(({ report }) => report.actions.length > 0).length,
      warningLimitExceeded,
    },
    exitCode: failures.length
      ? 2
      : errors || warningLimitExceeded || (policy.check && changed.length)
        ? 1
        : 0,
  };
}

/** Retains reports carried by failed rewrite planning in the same CLI envelope. */
export function failureResult(
  command: CliCommand,
  error: unknown,
  policy: ResultPolicy = {},
): CliResult {
  const results =
    error instanceof Error && "results" in error && Array.isArray(error.results)
      ? (error.results as LintResult["results"])
      : [];
  const result = cliResult("lint", { config: results[0]?.config ?? "", results }, policy);
  result.command = command;
  result.config = results[0]?.config ?? null;
  if (!results.length || !result.failures.length) {
    result.failures = [
      { message: error instanceof Error ? error.message : String(error), details: [] },
    ];
  }
  result.summary.failures = result.failures.length;
  result.exitCode = 2;
  return result;
}
