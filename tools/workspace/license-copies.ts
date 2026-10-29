/**
 * Copies license files from the repository root into the packages that need their
 * own copy. `--check` fails when a copy differs instead of writing it.
 *
 * `pnpm pack` adds the root `LICENSE` to a package without a license file, and it
 * skips symbolic links. The VS Code extension publishes `dist/extension/LICENSE`,
 * which stops that fallback, so it keeps a root-level copy. The packages that contain
 * material from Tailwind CSS or Webref keep copies of their licenses and list them in
 * `files`. The native binaries contain material from both.
 */

import { readdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";

const root = path.resolve(import.meta.dirname, "../..");
const platforms = path.join(root, "packages/native/npm");

const binaries = [
  "packages/vscode-extension",
  ...(await readdir(platforms, { withFileTypes: true }))
    .filter((entry) => entry.isDirectory())
    .map((entry) => `packages/native/npm/${entry.name}`),
];

/** Destination directories for each license file, relative to the repository root. */
const copies: Record<string, string[]> = {
  LICENSE: ["packages/vscode-extension"],
  "TAILWINDCSS-MIT-LICENSE": ["packages/core", ...binaries],
  "WEBREF-MIT-LICENSE": binaries,
};

const stale: string[] = [];
for (const [name, directories] of Object.entries(copies)) {
  const license = await readFile(path.join(root, name), "utf8");
  for (const directory of directories) {
    const destination = path.join(root, directory, name);
    if (process.argv.includes("--check")) {
      const current = await readFile(destination, "utf8").catch(() => undefined);
      if (current !== license) {
        stale.push(`${directory}/${name}`);
      }
    } else {
      await writeFile(destination, license);
    }
  }
}
if (stale.length) {
  throw new Error(`${stale.join(", ")} differ from the root. Run vp run generate:license-copies.`);
}
