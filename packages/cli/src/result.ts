import { lintReports } from "@stillwind/analysis";
import type { DesignSystemAnswer, InspectPreview, LintResult } from "@stillwind/analysis/node";

/**
 * A command that reads a design system and no source files.
 */
export type DesignCommand = "theme" | "utilities" | "variants";

export type CliCommand = "lint" | "fmt" | "inspect" | DesignCommand | null;

export type CliDiagnostic = ReturnType<typeof lintReports>[number];

export interface CliFailure {
  project?: number;
  file?: string;
  message: string;
  details: {
    file: string;
    message: string;
  }[];
}

export interface CliResult {
  command: CliCommand;
  config: string | null;
  diagnostics: CliDiagnostic[];
  failures: CliFailure[];
  changed: string[];
  previews: InspectPreview[];
  tokens: DesignSystemAnswer["tokens"];
  candidates: DesignSystemAnswer["candidates"];
  unmatched: string[];
  summary: {
    files: number;
    errors: number;
    warnings: number;
    failures: number;
    changed: number;
    previews: number;
    invalidPreviews: number;
    tokens: number;
    candidates: number;
    invalidCandidates: number;
    unmatched: number;
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

/**
 * Tests whether a command reads a design system and no source files.
 */
export function isDesignCommand(command: CliCommand): command is DesignCommand {
  return command === "theme" || command === "utilities" || command === "variants";
}

/**
 * Projects analysis into CLI data before visibility filtering and rendering.
 */
export function cliResult(
  command: CliCommand,
  result: Pick<LintResult, "config" | "results" | "changed"> & {
    failures?: readonly Pick<NonNullable<LintResult["failures"]>[number], "file" | "message">[];
    previews?: InspectPreview[];
  } & Partial<Pick<DesignSystemAnswer, "tokens" | "candidates" | "unmatched">>,
  policy: ResultPolicy = {},
): CliResult {
  const failedFiles = new Set(result.failures?.map((failure) => failure.file));
  const previews = result.previews ?? [];
  const invalid = previews.filter(({ valid }) => !valid).length;
  const { tokens = [], candidates = [], unmatched = [] } = result;
  const invalidCandidates = candidates.filter(({ valid }) => !valid).length;
  const diagnostics = lintReports(
    command !== "lint"
      ? result.results.filter(
          ({ result }) => result.status !== "complete" || failedFiles.has(result.entry),
        )
      : result.results,
  );
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
  for (const failure of result.failures ?? []) {
    if (!failures.some((existing) => existing.file === failure.file)) {
      failures.push({
        file: failure.file,
        message: failure.message,
        details: [],
      });
    }
  }
  if (!result.results.length && !failures.length && !isDesignCommand(command)) {
    failures.push({
      message: "No analysis targets matched the configured scopes",
      details: [],
    });
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
    previews,
    tokens,
    candidates,
    unmatched,
    summary: {
      files: new Set([
        ...result.results.map(({ result }) => result.entry),
        ...(result.failures ?? []).map((failure) => failure.file),
      ]).size,
      errors,
      warnings,
      failures: failures.length,
      changed: changed.length,
      previews: previews.length,
      invalidPreviews: invalid,
      tokens: tokens.length,
      candidates: candidates.length,
      invalidCandidates,
      unmatched: unmatched.length,
      suggestions: diagnostics.filter(({ report }) => report.actions.length > 0).length,
      warningLimitExceeded,
    },
    exitCode: failures.length
      ? 2
      : errors ||
          warningLimitExceeded ||
          (policy.check && changed.length) ||
          (command === "inspect" && (invalid || !previews.length)) ||
          invalidCandidates ||
          unmatched.length
        ? 1
        : 0,
  };
}

/**
 * Retains reports carried by failed rewrite planning in the same CLI envelope.
 */
export function failureResult(
  command: CliCommand,
  error: unknown,
  policy: ResultPolicy = {},
): CliResult {
  const results =
    error instanceof Error && "results" in error && Array.isArray(error.results)
      ? (error.results as LintResult["results"])
      : [];
  const result = cliResult(
    "lint",
    {
      config: results[0]?.config ?? "",
      results,
    },
    policy,
  );
  result.command = command;
  result.config = results[0]?.config ?? null;
  if (!results.length || !result.failures.length) {
    result.failures = [
      {
        message: error instanceof Error ? error.message : String(error),
        details: [],
      },
    ];
  }
  result.summary.failures = result.failures.length;
  result.exitCode = 2;
  return result;
}
