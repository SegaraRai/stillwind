import { readFile, writeFile } from "node:fs/promises";

/** A replacement with the original bytes retained for verification before saving. */
export interface FileChange {
  file: string;
  before: Buffer;
  after: Buffer;
}

/**
 * Saves changed bytes once per file. Identical buffers perform no filesystem operations.
 * Verifies all changed originals before any write and checks each again immediately before
 * saving. Throws when another writer changed a file or an I/O operation fails.
 */
export async function writeChangedFiles(changes: readonly FileChange[]): Promise<void> {
  const changed = changes.filter((change) => !change.before.equals(change.after));
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
