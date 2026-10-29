/**
 * Copies `RUST-THIRD-PARTY-LICENSES` into each platform package before publishing.
 * Git ignores the copies, and `generate-platform-packages.ts` lists the file in `files`.
 */
import { copyFile, readdir } from "node:fs/promises";
import path from "node:path";

const root = import.meta.dirname;
const platforms = path.join(root, "npm");

for (const entry of await readdir(platforms, { withFileTypes: true })) {
  if (entry.isDirectory()) {
    await copyFile(
      path.join(root, "RUST-THIRD-PARTY-LICENSES"),
      path.join(platforms, entry.name, "RUST-THIRD-PARTY-LICENSES"),
    );
  }
}
