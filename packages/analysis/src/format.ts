import type { WorkspaceResult } from "@stillwind/host";

import { rewriteFiles, type RewriteOptions } from "./rewrite-files.ts";
import type { RewriteFileFailure } from "./rewrite.ts";

/** Sorting results whose changed paths compare original and final source bytes. */
export interface FormatResult {
  /** Absolute path of the evaluated configuration. */
  config: string;
  /** Absolute paths that changed or would change, in sorted order. */
  changed: string[];
  /** Findings from the original sources, in configured project and target order. */
  results: WorkspaceResult[];
  /** Targets that could not be read, rewritten, or saved. */
  failures: RewriteFileFailure[];
  /** Failures return 2. Check mode otherwise returns 1 when any file would change. */
  exitCode: 0 | 1 | 2;
}

/**
 * Sorts configured targets and saves changed files unless check is true.
 * @param options - Configuration discovery, target filters, and optional check mode.
 * @returns Changed paths, original findings, and failures. Healthy targets continue after a failure.
 * @throws If selection, configuration, worker communication, or cleanup fails.
 */
export async function format(
  options: RewriteOptions & {
    /** Whether to report changes without saving them. Defaults to false. */
    check?: boolean;
  } = {},
): Promise<FormatResult> {
  const result = await rewriteFiles(options, "sort", !options.check);
  return {
    ...result,
    exitCode: result.failures.length ? 2 : options.check && result.changed.length ? 1 : 0,
  };
}
