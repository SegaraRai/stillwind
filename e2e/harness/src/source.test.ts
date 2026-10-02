import { mkdtemp, readFile, rm, stat, utimes, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { afterEach, beforeEach, expect, test } from "vite-plus/test";

import { snapshotSources } from "./source.ts";

let directory: string;
beforeEach(async () => {
  directory = await mkdtemp(join(tmpdir(), "stillwind-source-snapshot-"));
});
afterEach(async () => {
  await rm(directory, { recursive: true, force: true });
});

test("restores source bytes while retaining a production theme and its cache timestamp", async () => {
  const source = join(directory, "source.ts");
  const theme = join(directory, "root.css");
  await writeFile(source, "export const color = 'red';\r\n");
  await writeFile(theme, "original theme");
  const snapshot = await snapshotSources(directory, ["source.ts", "root.css"]);
  await writeFile(source, "changed source");
  await writeFile(theme, "production theme");
  const modified = (await stat(theme)).mtimeMs;
  await snapshot.reset(["root.css"]);
  expect(await readFile(source, "utf8")).toBe("export const color = 'red';\r\n");
  expect(await readFile(theme, "utf8")).toBe("production theme");
  expect((await stat(theme)).mtimeMs).toBe(modified);
  await snapshot.reset();
  expect(await readFile(theme, "utf8")).toBe("original theme");
});

test("keeps an unchanged source timestamp when a case restores its inputs", async () => {
  const source = join(directory, "source.ts");
  await writeFile(source, "original source");
  const timestamp = new Date("2020-01-01T00:00:00Z");
  await utimes(source, timestamp, timestamp);
  const snapshot = await snapshotSources(directory, ["source.ts"]);
  const original = (await stat(source)).mtimeMs;
  await snapshot.reset();
  expect((await stat(source)).mtimeMs).toBe(original);
});

test("recreates a captured file deleted by a case and keeps unrelated files", async () => {
  const source = join(directory, "source.ts");
  await writeFile(source, "original source");
  const snapshot = await snapshotSources(directory, ["source.ts"]);
  await rm(source);
  await writeFile(join(directory, "other.ts"), "other source");
  await snapshot.reset();
  expect(await readFile(source, "utf8")).toBe("original source");
  expect(await readFile(join(directory, "other.ts"), "utf8")).toBe("other source");
});
