import { createHash } from "node:crypto";
import {
  mkdtemp,
  readFile,
  readdir,
  realpath,
  rm,
  stat,
  utimes,
  writeFile,
} from "node:fs/promises";
import { dirname, join } from "node:path";

import { afterEach, beforeEach, expect, test, vi } from "vite-plus/test";

import {
  prepareTailwindSource,
  refreshTailwindSource,
  verifyTailwindSource,
  type TailwindSource,
} from "./upstream.ts";

let cacheDirectory: string;

beforeEach(async () => {
  cacheDirectory = await mkdtemp(join(import.meta.dirname, ".test-upstream-"));
});

afterEach(async () => {
  const directory = await realpath(cacheDirectory);
  expect(dirname(directory)).toBe(await realpath(import.meta.dirname));
  expect(directory).toContain(".test-upstream-");
  await rm(directory, { recursive: true, force: true });
});

function fixture(name = "tailwindcss", version = "4.3.3") {
  const files: Record<string, Uint8Array> = {
    "packages/tailwindcss/package.json": Buffer.from(JSON.stringify({ name, version })),
    "packages/tailwindcss/src/utilities.ts": Buffer.from("staticUtility('block', []);"),
    "packages/tailwindcss/src/variants.ts": Buffer.from("staticVariant('hover', []);"),
    "packages/tailwindcss/src/__snapshots__/intellisense.test.ts.snap": Buffer.from("snapshot"),
  };
  const source: TailwindSource = {
    version: "4.3.3",
    revision: "a".repeat(40),
    files: Object.fromEntries(
      Object.entries(files).map(([file, content]) => [
        file,
        createHash("sha256").update(content).digest("hex"),
      ]),
    ),
  };
  const download = vi.fn(async (url: string) => {
    const file = url.split(`/${source.revision}/`)[1];
    expect(file).toBeDefined();
    expect(files[file]).toBeDefined();
    return files[file];
  });
  return { source, files, download };
}

test("downloads pinned URLs and reuses verified files without network access", async () => {
  const { source, files, download } = fixture();
  const options = { source, cacheDirectory, expectedVersion: source.version, download };
  const directory = await prepareTailwindSource(options);
  expect(directory).toBe(join(cacheDirectory, source.revision));
  for (const [file, content] of Object.entries(files)) {
    expect(download).toHaveBeenCalledWith(
      `https://raw.githubusercontent.com/tailwindlabs/tailwindcss/${source.revision}/${file}`,
    );
    expect(await readFile(join(directory, file))).toEqual(content);
  }
  expect(await readdir(cacheDirectory)).toEqual([source.revision]);
  for (const file of Object.keys(files)) {
    await utimes(join(directory, file), 1, 1);
  }
  const offline = vi.fn(async () => expect.fail("A verified cache must not download files"));
  expect(await prepareTailwindSource({ ...options, download: offline })).toBe(directory);
  expect(offline).not.toHaveBeenCalled();
  for (const file of Object.keys(files)) {
    expect((await stat(join(directory, file))).mtimeMs).toBe(1000);
  }
});

test.each(["hash", "network"])(
  "deletes partial files after a %s failure and retries",
  async (failure) => {
    const { source, files, download } = fixture();
    const manifest = "packages/tailwindcss/package.json";
    download.mockImplementationOnce(async () => files[manifest]);
    download.mockImplementationOnce(async () => {
      expect(await readFile(join(cacheDirectory, source.revision, manifest))).toEqual(
        files[manifest],
      );
      if (failure === "network") {
        throw new Error("Network unavailable");
      }
      return Buffer.from("modified source");
    });
    await expect(
      prepareTailwindSource({ source, cacheDirectory, expectedVersion: source.version, download }),
    ).rejects.toThrow(failure === "hash" ? "hash mismatch" : "Network unavailable");
    expect(await readdir(cacheDirectory)).toEqual([]);
    const directory = await prepareTailwindSource({
      source,
      cacheDirectory,
      expectedVersion: source.version,
      download,
    });
    expect(() => verifyTailwindSource(directory, source, source.version)).not.toThrow();
  },
);

test.each(["modified", "incomplete"])(
  "rejects a %s cache without downloading or writing",
  async (state) => {
    const { source, download } = fixture();
    const options = { source, cacheDirectory, expectedVersion: source.version, download };
    const directory = await prepareTailwindSource(options);
    const file = join(directory, "packages/tailwindcss/src/utilities.ts");
    if (state === "modified") {
      await writeFile(file, "modified source");
    } else {
      await rm(file);
    }
    download.mockClear();
    expect(() => verifyTailwindSource(directory, source, source.version)).toThrow();
    await expect(prepareTailwindSource(options)).rejects.toThrow();
    expect(download).not.toHaveBeenCalled();
    expect(await readdir(cacheDirectory)).toEqual([source.revision]);
  },
);

test.each(["missing", "outdated"])(
  "creates the source pin from the installed release when JSON is %s",
  async (state) => {
    const latest = fixture("tailwindcss", "4.3.4");
    latest.source.revision = "b".repeat(40);
    const sourceFile = join(cacheDirectory, "source.json");
    if (state === "outdated") {
      await writeFile(
        sourceFile,
        JSON.stringify({
          version: "4.3.2",
          revision: "invalid",
          files: { "obsolete-file": "invalid" },
        }),
      );
    }
    const resolveRevision = vi.fn(async () => latest.source.revision);
    const updated = await refreshTailwindSource({
      sourceFile,
      cacheDirectory: join(cacheDirectory, "cache"),
      expectedVersion: "4.3.4",
      resolveRevision,
      download: latest.download,
    });
    expect(resolveRevision).toHaveBeenCalledWith("4.3.4");
    expect(latest.download).toHaveBeenCalledTimes(Object.keys(latest.files).length);
    expect(updated).toEqual({ ...latest.source, version: "4.3.4" });
    expect(JSON.parse(await readFile(sourceFile, "utf8"))).toEqual(updated);
    expect(() =>
      verifyTailwindSource(join(cacheDirectory, "cache", updated.revision), updated, "4.3.4"),
    ).not.toThrow();
  },
);

test.each(["download", "version", "revision"])(
  "preserves the source JSON after refresh %s failure",
  async (failure) => {
    const previous = fixture().source;
    const latest = fixture("tailwindcss", failure === "version" ? "4.3.2" : "4.3.4");
    latest.source.revision = "b".repeat(40);
    const sourceFile = join(cacheDirectory, "source.json");
    const original = JSON.stringify(previous);
    await writeFile(sourceFile, original);
    if (failure === "download") {
      latest.download.mockRejectedValueOnce(new Error("Network unavailable"));
    }
    await expect(
      refreshTailwindSource({
        sourceFile,
        cacheDirectory: join(cacheDirectory, "cache"),
        expectedVersion: "4.3.4",
        resolveRevision: async () => (failure === "revision" ? "v4.3.4" : latest.source.revision),
        download: latest.download,
      }),
    ).rejects.toThrow();
    expect(await readFile(sourceFile, "utf8")).toBe(original);
  },
);

test.each([
  { name: "tailwindcss", version: "4.3.2" },
  { name: "another-package", version: "4.3.3" },
])(
  "deletes downloaded files for invalid upstream package $name $version",
  async ({ name, version }) => {
    const { source, download } = fixture(name, version);
    await expect(
      prepareTailwindSource({ source, cacheDirectory, expectedVersion: source.version, download }),
    ).rejects.toThrow("Expected Tailwind 4.3.3 source");
    expect(await readdir(cacheDirectory)).toEqual([]);
  },
);

test("requires a source pin matching the installed package before downloading", async () => {
  const { source, download } = fixture();
  await expect(
    prepareTailwindSource({ source, cacheDirectory, expectedVersion: "4.3.4", download }),
  ).rejects.toThrow("update upstream-source.json");
  expect(download).not.toHaveBeenCalled();
  expect(await readdir(cacheDirectory)).toEqual([]);
});

test.each(["revision", "path", "hash"])(
  "rejects an invalid %s before downloading",
  async (field) => {
    const { source, download } = fixture();
    if (field === "revision") {
      source.revision = "v4.3.3";
    } else if (field === "path") {
      source.files["packages/tailwindcss/../../../outside"] = "a".repeat(64);
    } else {
      source.files["packages/tailwindcss/package.json"] = "not-a-hash";
    }
    await expect(
      prepareTailwindSource({ source, cacheDirectory, expectedVersion: source.version, download }),
    ).rejects.toThrow();
    expect(download).not.toHaveBeenCalled();
    expect(await readdir(cacheDirectory)).toEqual([]);
  },
);
