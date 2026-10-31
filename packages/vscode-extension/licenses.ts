/**
 * Renders the licenses of the npm packages that the extension includes.
 *
 * Each runtime bundle configuration adds {@link recordBundledPackages}, which records
 * the package directories of its bundled modules in `dist/licenses/`. `build.ts` then
 * calls {@link writeExtensionLicenses} to combine the records into one license file.
 */

import { mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";

import {
  bundledPackages,
  renderNpmLicenses,
} from "@stillwind-internal/tools-workspace/third-party-licenses";

const root = import.meta.dirname;
const records = path.join(root, "dist/licenses");

/**
 * Returns a Rolldown plugin that writes the npm package directories of the bundle to
 * `dist/licenses/<name>.json`. The paths are relative to this package because Desktop
 * packaging can read the records in a different checkout directory.
 *
 * @param name A name that is unique among the bundle configurations.
 */
export function recordBundledPackages(name: string) {
  return bundledPackages((directories) => {
    mkdirSync(records, { recursive: true });
    writeFileSync(
      path.join(records, `${name}.json`),
      JSON.stringify(directories.map((directory) => path.relative(root, directory))),
    );
  });
}

/**
 * Writes `NPM-THIRD-PARTY-LICENSES` into `output` for the recorded packages and for
 * `extra` package directories. `extra` covers packages whose files the build copies
 * without bundling, such as WebAssembly binaries.
 *
 * Throws when no bundle configuration has recorded its packages yet.
 */
export function writeExtensionLicenses(output: string, extra: string[]) {
  const files = readdirSync(records).filter((file) => file.endsWith(".json"));
  if (!files.length) {
    throw new Error(`${records} has no package records. Run vp pack first.`);
  }
  const directories = new Set(extra);
  for (const file of files) {
    for (const directory of JSON.parse(readFileSync(path.join(records, file), "utf8"))) {
      directories.add(path.resolve(root, directory));
    }
  }
  writeFileSync(
    path.join(output, "NPM-THIRD-PARTY-LICENSES"),
    renderNpmLicenses(
      "The Stillwind VS Code extension includes the following npm packages.",
      directories,
    ),
  );
}
