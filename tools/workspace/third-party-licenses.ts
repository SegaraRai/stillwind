/**
 * Collects and renders the license texts of third-party packages that published
 * artifacts include.
 *
 * {@link bundledPackages} finds the npm packages in a Rolldown bundle, and
 * {@link npmPackage} reads one of them. The Rust crate generator in `packages/native`
 * builds its entries from Cargo metadata. {@link renderThirdPartyLicenses} renders the
 * entries into one file.
 */

import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";

/**
 * A third-party package that a published artifact includes.
 */
export interface ThirdPartyPackage {
  name: string;
  version: string;
  /** SPDX license expression that the package declares. */
  license: string;
  repository?: string;
  /** Names without email addresses. The MIT fallback text uses them as copyright holders. */
  authors: string[];
  /** Texts of the license files in the package, in file-name order. Empty when it has none. */
  texts: string[];
}

const licenseFile = /^(licen[cs]e|copying|notice|unlicense|third[-_]?party)/i;

/**
 * Returns the texts of the license and notice files directly in `directory`, sorted by
 * file name, with line endings normalized to LF.
 */
export function licenseTexts(directory: string): string[] {
  return readdirSync(directory, { withFileTypes: true })
    .filter((entry) => entry.isFile() && licenseFile.test(entry.name))
    .map((entry) => entry.name)
    .sort()
    .map((name) => normalize(readFileSync(path.join(directory, name), "utf8")));
}

/**
 * Removes email addresses and URLs in angle brackets from an author entry.
 */
export function authorName(author: string) {
  return author.replace(/\s*[<(][^>)]*[>)]/g, "").trim();
}

function normalize(text: string) {
  return `${text
    .replaceAll("\r\n", "\n")
    .split("\n")
    .map((line) => line.trimEnd())
    .join("\n")
    .trim()}\n`;
}

const standardTexts = path.join(import.meta.dirname, "licenses");

/**
 * Returns the standard text for a package without license files. A choice such as
 * `MIT OR Apache-2.0` uses MIT when it is one of the options. The MIT text names the
 * package authors, or "The <name> authors" when the package lists none. Throws for
 * other licenses, so that a new dependency without license files needs a review.
 */
function fallbackText(entry: ThirdPartyPackage) {
  const options = entry.license.split(/\s+OR\s+|\//).map((option) => option.trim());
  if (options.includes("MIT")) {
    const holders = entry.authors.length ? entry.authors.join(", ") : `The ${entry.name} authors`;
    return normalize(
      readFileSync(path.join(standardTexts, "MIT.txt"), "utf8").replace("{holders}", holders),
    );
  }
  if (options.includes("MPL-2.0")) {
    return normalize(readFileSync(path.join(standardTexts, "MPL-2.0.txt"), "utf8"));
  }
  throw new Error(
    `${entry.name} ${entry.version} has no license file, and ${entry.license} has no standard text.`,
  );
}

/**
 * Renders `packages` as plain text. Each distinct license text appears once, after the
 * packages that include it, so a package with several license files appears in several
 * sections. The output is sorted by package name and version, so it depends only on its
 * input.
 *
 * @param heading The first paragraph, which names the artifact that includes the packages.
 */
export function renderThirdPartyLicenses(heading: string, packages: ThirdPartyPackage[]) {
  const groups = new Map<string, ThirdPartyPackage[]>();
  const sorted = [...packages].sort(
    (a, b) => a.name.localeCompare(b.name, "en") || a.version.localeCompare(b.version, "en"),
  );
  for (const entry of sorted) {
    for (const text of entry.texts.length ? entry.texts : [fallbackText(entry)]) {
      groups.set(text, [...(groups.get(text) ?? []), entry]);
    }
  }
  const separator = "=".repeat(80);
  const sections = [...groups].map(([text, entries]) => {
    const names = entries.map(
      (entry) =>
        `${entry.name} ${entry.version} (${entry.license})${entry.repository ? ` ${entry.repository}` : ""}`,
    );
    return `${separator}\n${names.join("\n")}\n${separator}\n\n${text}`;
  });
  return `${heading.trim()}\n\n${sections.join("\n")}`;
}

const nodeModules = `${path.sep}node_modules${path.sep}`;

/**
 * Returns the directory of the npm package that contains `id`, or `undefined` for
 * virtual modules and workspace sources. Workspace packages are covered by the
 * Stillwind license.
 */
function packageDirectory(id: string) {
  if (!path.isAbsolute(id)) {
    return undefined;
  }
  const file = path.normalize(id.split("?")[0]!);
  const start = file.lastIndexOf(nodeModules);
  if (start < 0) {
    return undefined;
  }
  const rest = file.slice(start + nodeModules.length).split(path.sep);
  const name = rest[0]!.startsWith("@") ? rest.slice(0, 2) : rest.slice(0, 1);
  return path.join(file.slice(0, start + nodeModules.length), ...name);
}

/**
 * Returns a Rolldown plugin that passes the sorted directories of the npm packages in
 * the generated chunks to `onPackages`. Workspace packages are excluded. The plugin
 * calls `onPackages` once for each generated bundle.
 */
export function bundledPackages(
  onPackages: (directories: string[], outputDirectory: string | undefined) => void,
) {
  return {
    name: "stillwind:bundled-packages",
    generateBundle(
      options: { dir?: string },
      bundle: Record<string, { type: string; moduleIds?: readonly string[] }>,
    ) {
      const directories = new Set<string>();
      for (const output of Object.values(bundle)) {
        for (const id of output.type === "chunk" ? (output.moduleIds ?? []) : []) {
          const directory = packageDirectory(id);
          if (directory) {
            directories.add(directory);
          }
        }
      }
      onPackages([...directories].sort(), options.dir);
    },
  };
}

/**
 * Reads the manifest and the license files of the npm package in `directory`.
 */
export function npmPackage(directory: string): ThirdPartyPackage {
  const manifest = JSON.parse(readFileSync(path.join(directory, "package.json"), "utf8")) as {
    name: string;
    version: string;
    license?: string;
    author?: string | { name: string };
    contributors?: (string | { name: string })[];
    repository?: string | { url: string };
  };
  const people = [manifest.author, ...(manifest.contributors ?? [])].filter(
    (person) => person !== undefined,
  );
  const repository =
    typeof manifest.repository === "string" ? manifest.repository : manifest.repository?.url;
  return {
    name: manifest.name,
    version: manifest.version,
    license: manifest.license ?? "unknown",
    repository: repository?.replace(/^git\+/, ""),
    authors: people
      .map((person) => authorName(typeof person === "string" ? person : person.name))
      .filter(Boolean),
    texts: licenseTexts(directory),
  };
}

/**
 * Reads the npm packages in `directories` and renders their licenses. Directories of
 * the same package name and version produce one entry.
 */
export function renderNpmLicenses(heading: string, directories: Iterable<string>) {
  const packages = new Map<string, ThirdPartyPackage>();
  for (const directory of directories) {
    const entry = npmPackage(directory);
    packages.set(`${entry.name}@${entry.version}`, entry);
  }
  return renderThirdPartyLicenses(heading, [...packages.values()]);
}
