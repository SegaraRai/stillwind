import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

import { lintReports } from "@stillwind/analysis";
import type {
  DesignSystemAnswer,
  InspectEmptyReason,
  InspectPreview,
  LintResult,
  ProjectInfo,
} from "@stillwind/analysis/node";

import { documentationPage, stillwindRoot } from "./documentation.ts";

/**
 * A command that reads a design system and no source files.
 */
export type DesignCommand = "theme" | "utilities" | "variants";

export type CliCommand = "check" | "lint" | "fmt" | "inspect" | "info" | DesignCommand | null;

type RawReport = ReturnType<typeof lintReports>[number]["report"];

/** One suggested edit action annotated with whether `--fix` applies it automatically. */
export type CliAction = RawReport["actions"][number] & {
  /** True when `--fix` applies this action automatically; false for manual suggestions. */
  autofix: boolean;
};

export type CliDiagnostic = Omit<ReturnType<typeof lintReports>[number], "report"> & {
  /** Workspace-relative POSIX path of the primary location (or absolute on another volume). */
  file: string;
  /** One-based start line of the primary location when scope is `"range"`. */
  line?: number;
  /** One-based UTF-16 start column of the primary location when scope is `"range"`. */
  column?: number;
  report: Omit<RawReport, "actions"> & {
    actions: CliAction[];
  };
  /** Absolute path or URL of the page that explains the report code. */
  docs?: string;
};

/** A preview whose findings link to the pages that explain their codes. */
export type CliPreview = Omit<InspectPreview, "diagnostics"> & {
  /** Workspace-relative POSIX path of the preview location (or absolute on another volume). */
  file: string;
  /** One-based start line of the preview range. */
  line: number;
  /** One-based UTF-16 start column of the preview range. */
  column: number;
  diagnostics: (InspectPreview["diagnostics"][number] & {
    /** Absolute path or URL of the page that explains the code. */
    docs?: string;
  })[];
};

/** Aggregated counts for one diagnostic code or lint rule. */
export interface CodeSummary {
  errors: number;
  warnings: number;
  autofixable: number;
  /** Absolute path or URL of the page that explains this code, when available. */
  docs?: string;
}

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
  previews: CliPreview[];
  /** Explains why `inspect` produced no previews without failures. Omitted otherwise. */
  emptyReason?: InspectEmptyReason;
  projects: ProjectInfo[];
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
    projects: number;
    tokens: number;
    candidates: number;
    invalidCandidates: number;
    unmatched: number;
    /** Diagnostics that have an automatic fix applied by `--fix`. */
    autofixable: number;
    /** Diagnostics that have manual suggested edits not applied by `--fix`. */
    suggestions: number;
    /** Number of diagnostics omitted from the `diagnostics` array by `--limit`. */
    omittedDiagnostics: number;
    /** Counts grouped by diagnostic code or lint rule, in code order. */
    byCode: Record<string, CodeSummary>;
    warningLimitExceeded: boolean;
  };
  exitCode: 0 | 1 | 2;
}

export interface ResultPolicy {
  check?: boolean;
  quiet?: boolean;
  maxWarnings?: number;
  limit?: number;
  cwd?: string;
}

function relativeFilePath(uri: string, cwd: string): string {
  let file = uri;
  if (uri.startsWith("file:")) {
    try {
      file = fileURLToPath(uri);
    } catch {
      file = uri;
    }
  }
  const relative = path.isAbsolute(file) ? path.relative(cwd, file) || "." : file;
  return relative.replaceAll("\\", "/");
}

/**
 * Tests whether a command reads a design system and no source files.
 */
export function isDesignCommand(command: CliCommand): command is DesignCommand {
  return command === "theme" || command === "utilities" || command === "variants";
}

/**
 * Returns whether `--fix` automatically applies `action` for `report`.
 * Only the first action whose edits are nonempty and stay on the primary document's revision
 * is applied automatically; subsequent alternatives or cross-document edits are manual suggestions.
 */
function isAutofixAction(
  report: RawReport,
  action: RawReport["actions"][number],
  index: number,
): boolean {
  return (
    index === 0 &&
    action.edits.length > 0 &&
    action.edits.every(
      (edit) =>
        edit.location.uri === report.primary.location.uri &&
        edit.location.revision === report.primary.location.revision,
    )
  );
}

/**
 * Projects analysis into CLI data before visibility filtering and rendering.
 */
export function cliResult(
  command: CliCommand,
  result: Pick<LintResult, "config" | "results" | "changed"> & {
    failures?: readonly Pick<NonNullable<LintResult["failures"]>[number], "file" | "message">[];
    previews?: InspectPreview[];
    emptyReason?: InspectEmptyReason;
    projects?: ProjectInfo[];
  } & Partial<Pick<DesignSystemAnswer, "tokens" | "candidates" | "unmatched">>,
  policy: ResultPolicy = {},
): CliResult {
  const failedFiles = new Set(result.failures?.map((failure) => failure.file));
  const cwd = policy.cwd ?? process.cwd();
  const root = stillwindRoot(result.config);
  const docs = (code: string) => {
    const page = documentationPage(code, root);
    return page ? { docs: page } : {};
  };
  const previews: CliPreview[] = (result.previews ?? []).map((preview) => ({
    ...preview,
    file: relativeFilePath(preview.location.uri, cwd),
    line: preview.location.startLine + 1,
    column: preview.location.startColumn + 1,
    diagnostics: preview.diagnostics.map((diagnostic) => ({
      ...diagnostic,
      ...docs(diagnostic.code),
    })),
  }));
  const invalid = previews.filter(({ valid }) => !valid).length;
  const { tokens = [], candidates = [], unmatched = [], projects = [] } = result;
  const invalidCandidates = candidates.filter(({ valid }) => !valid).length;
  const diagnostics: CliDiagnostic[] = lintReports(
    command !== "lint" && command !== "check"
      ? result.results.filter(
          ({ result }) => result.status !== "complete" || failedFiles.has(result.entry),
        )
      : result.results,
  ).map((diagnostic) => {
    const { primary } = diagnostic.report;
    return {
      ...diagnostic,
      file: relativeFilePath(primary.location.uri, cwd),
      ...(primary.scope === "range"
        ? {
            line: primary.location.startLine + 1,
            column: primary.location.startColumn + 1,
          }
        : {}),
      report: {
        ...diagnostic.report,
        actions: diagnostic.report.actions.map((action, index) => ({
          ...action,
          autofix: isAutofixAction(diagnostic.report, action, index),
        })),
      },
      ...docs(diagnostic.report.code),
    };
  });
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
  if (
    !result.results.length &&
    !failures.length &&
    !isDesignCommand(command) &&
    command !== "info"
  ) {
    failures.push({
      message:
        "No analysis targets matched the configured scopes; check `files`/`exclude` in stillwind.config.* or the provided glob patterns",
      details: [],
    });
  }
  const changed = [...new Set(result.changed ?? [])].sort();
  const errors = diagnostics.filter(({ report }) => report.severity === "error").length;
  const warnings = diagnostics.length - errors;
  const warningLimitExceeded = policy.maxWarnings !== undefined && warnings > policy.maxWarnings;
  const byCodeMap = new Map<string, CodeSummary>();
  for (const diagnostic of diagnostics) {
    const { code, severity, actions } = diagnostic.report;
    const entry = byCodeMap.get(code) ?? {
      errors: 0,
      warnings: 0,
      autofixable: 0,
      ...(diagnostic.docs ? { docs: diagnostic.docs } : {}),
    };
    if (severity === "error") {
      entry.errors += 1;
    } else {
      entry.warnings += 1;
    }
    if (actions.some((action) => action.autofix)) {
      entry.autofixable += 1;
    }
    byCodeMap.set(code, entry);
  }
  const byCode: Record<string, CodeSummary> = {};
  for (const code of [...byCodeMap.keys()].sort()) {
    byCode[code] = byCodeMap.get(code)!;
  }
  const visible = policy.quiet
    ? diagnostics.filter(({ report }) => report.severity === "error")
    : diagnostics;
  const shown = policy.limit === undefined ? visible : visible.slice(0, policy.limit);
  const omittedDiagnostics = visible.length - shown.length;
  return {
    command,
    config: result.config,
    diagnostics: shown,
    failures,
    changed,
    previews,
    ...(result.emptyReason ? { emptyReason: result.emptyReason } : {}),
    projects,
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
      projects: projects.length,
      tokens: tokens.length,
      candidates: candidates.length,
      invalidCandidates,
      unmatched: unmatched.length,
      autofixable: diagnostics.filter(({ report }) =>
        report.actions.some((action) => action.autofix),
      ).length,
      suggestions: diagnostics.filter(({ report }) =>
        report.actions.some((action) => !action.autofix),
      ).length,
      omittedDiagnostics,
      byCode,
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
