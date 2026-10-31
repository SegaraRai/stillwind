/**
 * Renders the licenses of the npm packages that the extension includes.
 *
 * Each runtime bundle configuration adds {@link recordBundledPackages}, which records
 * the licenses of its bundled packages in `dist/licenses/`. `build.ts` then
 * calls {@link writeExtensionLicenses} to combine the records into one license file.
 */

import { mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";

import {
  bundledPackages,
  npmPackage,
  renderThirdPartyLicenses,
  type ThirdPartyPackage,
} from "@stillwind-internal/tools-workspace/third-party-licenses";

const records = path.join(import.meta.dirname, "dist/licenses");

/**
 * Returns a Rolldown plugin that writes the npm package licenses of the bundle to
 * `dist/licenses/<name>.json`. The records contain the license data because pnpm names
 * package directories differently on each platform, and Desktop packaging can run on a
 * different platform from the bundling.
 *
 * @param name A name that is unique among the bundle configurations.
 */
export function recordBundledPackages(name: string) {
  return bundledPackages((directories) => {
    mkdirSync(records, { recursive: true });
    writeFileSync(path.join(records, `${name}.json`), JSON.stringify(directories.map(npmPackage)));
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
  const packages = new Map<string, ThirdPartyPackage>();
  for (const entry of [
    ...extra.map(npmPackage),
    ...files.flatMap((file): ThirdPartyPackage[] =>
      JSON.parse(readFileSync(path.join(records, file), "utf8")),
    ),
  ]) {
    packages.set(`${entry.name}@${entry.version}`, entry);
  }
  writeFileSync(
    path.join(output, "NPM-THIRD-PARTY-LICENSES"),
    renderThirdPartyLicenses(
      "The Stillwind VS Code extension includes the following npm packages.",
      [...packages.values()],
    ),
  );
}
