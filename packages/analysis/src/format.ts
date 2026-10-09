import type { WorkspaceResult } from "@stillwind/host";

import { rewriteFiles, type RewriteOptions } from "./rewrite-files.ts";

/** Sorting results whose changed paths compare original and final source bytes. */
export interface FormatResult {
  /** Absolute path of the evaluated configuration. */
  config: string;
  /** Absolute paths that changed or would change, in sorted order. */
  changed: string[];
  /** Findings from the original sources, in configured project and target order. */
  results: WorkspaceResult[];
  /** Check mode returns 1 when any file would change. Saving returns 0 after success. */
  exitCode: 0 | 1;
}

/**
 * Sorts configured targets and saves changed files unless check is true.
 * @param options - Configuration discovery, target filters, and optional check mode.
 * @returns Changed paths and original findings, with exitCode 1 only when check mode finds changes.
 * @throws If selection, configuration, planning, saving, or worker cleanup fails.
 */
export async function format(
  options: RewriteOptions & {
    /** Whether to report changes without saving them. Defaults to false. */
    check?: boolean;
  } = {},
): Promise<FormatResult> {
  const result = await rewriteFiles(options, "sort", !options.check);
  return { ...result, exitCode: options.check && result.changed.length ? 1 : 0 };
}
