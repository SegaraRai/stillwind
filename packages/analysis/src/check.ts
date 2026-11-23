import type { WorkspaceResult } from "@stillwind/host";

import { rewriteFiles, type RewriteOptions } from "./rewrite-files.ts";
import type { RewriteFileFailure } from "./rewrite.ts";

/**
 * Combined lint and formatting check results for one configuration.
 */
export interface CheckResult {
  /** Absolute path of the evaluated configuration. */
  config: string;
  /** Absolute paths that changed when fix is true, or need sorting when fix is false. */
  changed: string[];
  /** Findings from original sources without fix, or from final sorted sources with fix. */
  results: WorkspaceResult[];
  /** Targets that could not be read, rewritten, or saved. */
  failures: RewriteFileFailure[];
  /**
   * - 0: no failures, no errors, and no unformatted files when fix is false
   * - 1: compiler errors, denied lint, or unformatted files when fix is false
   * - 2: incomplete execution or target failures
   */
  exitCode: 0 | 1 | 2;
}

/**
 * Runs lint and utility sorting in one worker pass. Without fix, checks formatting and reports
 * findings without saving. With fix, applies converged automatic fixes and utility sorting, saves
 * changed files, and reports findings for the final sources.
 *
 * @param options - Configuration discovery, target filters, and optional fix mode.
 * @returns Changed paths, findings, failures, and exit code. Healthy targets continue after a failure.
 * @throws If selection, configuration, worker communication, or cleanup fails.
 */
export async function check(
  options: RewriteOptions & {
    /** Applies converged automatic fixes and utility sorting, and saves changed files. */
    fix?: boolean;
  } = {},
): Promise<CheckResult> {
  const result = await rewriteFiles(
    options,
    options.fix ? "fixes-and-sort" : "sort",
    !!options.fix,
  );
  const hasErrors = result.results.some(({ result: entry }) =>
    entry.reports.some((report) => report.severity === "error"),
  );
  return {
    ...result,
    exitCode: result.failures.length
      ? 2
      : hasErrors || (!options.fix && result.changed.length)
        ? 1
        : 0,
  };
}
