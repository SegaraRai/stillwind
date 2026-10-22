import { readFile, writeFile } from "node:fs/promises";

import type { RewriteFileFailure } from "./rewrite.ts";

/** A replacement with the original bytes retained for verification before saving. */
export interface FileChange {
  /** Target path. The caller must provide at most one change per file. */
  file: string;
  /** Bytes captured before planning, compared against disk before saving. */
  before: Buffer;
  /** Planned replacement bytes. Equal bytes skip verification and writing. */
  after: Buffer;
}

/**
 * Saves changed bytes once per file. Identical buffers perform no filesystem operations.
 * Checks each original immediately before saving and continues after a target fails.
 * @param changes - Distinct target paths and original/replacement bytes. Buffers remain unchanged.
 * @returns Saved paths and failures. A stale file is left unchanged.
 */
export async function writeChangedFiles(changes: readonly FileChange[]): Promise<{
  changed: string[];
  failures: RewriteFileFailure[];
}> {
  const changed = changes.filter((change) => !change.before.equals(change.after));
  const saved: string[] = [];
  const failures: RewriteFileFailure[] = [];
  for (const change of changed) {
    try {
      if (!(await readFile(change.file)).equals(change.before)) {
        throw new Error(`Source changed before saving ${change.file}`);
      }
      await writeFile(change.file, change.after);
      saved.push(change.file);
    } catch (error) {
      failures.push({
        file: change.file,
        message: error instanceof Error ? error.message : String(error),
        results: [],
      });
    }
  }
  return {
    changed: saved,
    failures,
  };
}
