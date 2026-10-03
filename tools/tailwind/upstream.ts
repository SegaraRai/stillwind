import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { mkdir, rm, stat, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { promisify } from "node:util";

const tailwindSourceFiles = [
  "packages/tailwindcss/package.json",
  "packages/tailwindcss/src/utilities.ts",
  "packages/tailwindcss/src/variants.ts",
  "packages/tailwindcss/src/__snapshots__/intellisense.test.ts.snap",
];

export interface TailwindSource {
  version: string;
  /** Full Git commit SHA used in download URLs and cache directory names. */
  revision: string;
  /** Repository-relative paths mapped to SHA-256 hashes of their bytes. */
  files: Record<string, string>;
}

interface PrepareTailwindSourceOptions {
  source: TailwindSource;
  cacheDirectory: string;
  expectedVersion: string;
  download?: (url: string) => Promise<Uint8Array>;
}

interface RefreshTailwindSourceOptions extends Omit<PrepareTailwindSourceOptions, "source"> {
  sourceFile: string;
  resolveRevision?: (version: string) => Promise<string>;
}

/**
 * Resolves the installed version's release tag, downloads the pinned files, and
 * updates the source JSON after validating and preparing the downloaded source.
 * Keeps the JSON unchanged if acquisition or verification fails. Callers must
 * serialize preparation of the same cache. Throws on invalid pins or versions,
 * tag resolution, download, verification, or file write failures.
 */
export async function refreshTailwindSource({
  sourceFile,
  cacheDirectory,
  expectedVersion,
  resolveRevision = resolveTailwindRevision,
  download = downloadTailwindFile,
}: RefreshTailwindSourceOptions): Promise<TailwindSource> {
  if (!/^\d+\.\d+\.\d+(?:-[\w.-]+)?(?:\+[\w.-]+)?$/.test(expectedVersion)) {
    throw new Error(`Invalid Tailwind package version: ${expectedVersion}`);
  }
  const updated: TailwindSource = {
    version: expectedVersion,
    revision: await resolveRevision(expectedVersion),
    files: {},
  };
  validateRevision(updated.revision);
  const contents = new Map<string, Uint8Array>();
  for (const file of tailwindSourceFiles) {
    const url = sourceUrl(updated.revision, file);
    const content = await download(url);
    contents.set(url, content);
    updated.files[file] = createHash("sha256").update(content).digest("hex");
  }
  await prepareTailwindSource({
    source: updated,
    cacheDirectory,
    expectedVersion,
    download: async (url) => contents.get(url)!,
  });
  await writeFile(sourceFile, `${JSON.stringify(updated, null, 2)}\n`);
  return updated;
}

async function resolveTailwindRevision(version: string): Promise<string> {
  const tag = `refs/tags/v${version}`;
  const { stdout } = await promisify(execFile)("git", [
    "ls-remote",
    "https://github.com/tailwindlabs/tailwindcss.git",
    tag,
    `${tag}^{}`,
  ]);
  const refs = new Map(
    stdout
      .trim()
      .split(/\r?\n/)
      .map((line) => {
        const [revision, ref] = line.split(/\s+/);
        return [ref, revision] as const;
      }),
  );
  const revision = refs.get(`${tag}^{}`) ?? refs.get(tag);
  if (!revision) {
    throw new Error(`Tailwind release tag v${version} was not found`);
  }
  return revision;
}

/**
 * Returns a directory containing the pinned files for the installed Tailwind version.
 * Reuses a verified cache without network access. Writes downloaded files directly
 * and deletes newly downloaded files on failure. Callers must serialize preparation of
 * the same cache. Throws on invalid pins, downloads, file hashes, or an incomplete
 * or modified existing cache.
 */
export async function prepareTailwindSource({
  source,
  cacheDirectory,
  expectedVersion,
  download = downloadTailwindFile,
}: PrepareTailwindSourceOptions): Promise<string> {
  validateSource(source, expectedVersion);
  const cache = resolve(cacheDirectory);
  const directory = join(cache, source.revision);
  if (dirname(directory) !== cache) {
    throw new Error(`Unexpected Tailwind cache directory: ${directory}`);
  }
  if (await directoryExists(directory)) {
    verifyTailwindSource(directory, source, expectedVersion);
    return directory;
  }
  await mkdir(directory, { recursive: true });
  try {
    for (const [file, expectedHash] of Object.entries(source.files)) {
      const content = await download(sourceUrl(source.revision, file));
      verifyHash(content, expectedHash, file);
      const destination = join(directory, file);
      await mkdir(dirname(destination), { recursive: true });
      await writeFile(destination, content);
    }
    verifyTailwindSource(directory, source, expectedVersion);
    return directory;
  } catch (error) {
    await rm(directory, { recursive: true, force: true });
    throw error;
  }
}

function sourceUrl(revision: string, file: string): string {
  return `https://raw.githubusercontent.com/tailwindlabs/tailwindcss/${revision}/${file}`;
}

async function directoryExists(directory: string): Promise<boolean> {
  try {
    await stat(directory);
    return true;
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "ENOENT") {
      throw error;
    }
    return false;
  }
}

async function downloadTailwindFile(url: string): Promise<Uint8Array> {
  const response = await fetch(url, { signal: AbortSignal.timeout(30000) });
  if (!response.ok) {
    throw new Error(`Tailwind source download failed: HTTP ${response.status} for ${url}`);
  }
  return new Uint8Array(await response.arrayBuffer());
}

/** Throws unless every pinned file matches its hash and the installed package version. */
export function verifyTailwindSource(
  directory: string,
  source: TailwindSource,
  expectedVersion: string,
): void {
  validateSource(source, expectedVersion);
  for (const [file, expectedHash] of Object.entries(source.files)) {
    verifyHash(readFileSync(join(directory, file)), expectedHash, file);
  }
  const manifestPath = join(directory, "packages/tailwindcss/package.json");
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8")) as {
    name?: string;
    version?: string;
  };
  if (manifest.name !== "tailwindcss" || manifest.version !== expectedVersion) {
    throw new Error(
      `Expected Tailwind ${expectedVersion} source; found ${manifest.name} ${manifest.version} at ${manifestPath}`,
    );
  }
}

function validateSource(source: TailwindSource, expectedVersion: string): void {
  if (source.version !== expectedVersion) {
    throw new Error(
      `Tailwind source pin is ${source.version}, but the installed package is ${expectedVersion}; update upstream-source.json`,
    );
  }
  validateRevision(source.revision);
  if (!("packages/tailwindcss/package.json" in source.files)) {
    throw new Error("Tailwind source pin must include packages/tailwindcss/package.json");
  }
  for (const [file, hash] of Object.entries(source.files)) {
    if (
      !file.startsWith("packages/tailwindcss/") ||
      file.split("/").some((part) => part === ".." || part === "." || part === "") ||
      /[\\\0:]/.test(file) ||
      !/^[a-f0-9]{64}$/.test(hash)
    ) {
      throw new Error(`Invalid Tailwind source pin for ${file}`);
    }
  }
}

function validateRevision(revision: string): void {
  if (!/^[a-f0-9]{40}$/.test(revision)) {
    throw new Error("Tailwind source revision must be a full Git commit SHA");
  }
}

function verifyHash(content: Uint8Array, expectedHash: string, file: string): void {
  if (createHash("sha256").update(content).digest("hex") !== expectedHash) {
    throw new Error(`Tailwind source hash mismatch: ${file}`);
  }
}
