import { readFile, writeFile } from "node:fs/promises";

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
 * Verifies all changed originals before any write and checks each again immediately before
 * saving.
 * @param changes - Distinct target paths and original/replacement bytes. Buffers remain unchanged.
 * @returns Completion of verification and sequential writes. Earlier writes remain if a later write fails.
 * @throws If current bytes differ from the original or a filesystem operation fails.
 */
export async function writeChangedFiles(changes: readonly FileChange[]): Promise<void> {
  const changed = changes.filter((change) => !change.before.equals(change.after));
  /**
   * Checks that a planned replacement still targets the bytes captured before rewriting.
   * @param change - Path and original bytes to compare with disk.
   * @returns Completion of the byte comparison.
   * @throws If reading fails or another writer changed the original.
   */
  const verify = async (change: FileChange) => {
    if (!(await readFile(change.file)).equals(change.before)) {
      throw new Error(`Source changed before saving ${change.file}`);
    }
  };
  await Promise.all(changed.map(verify));
  for (const change of changed) {
    await verify(change);
    await writeFile(change.file, change.after);
  }
}
