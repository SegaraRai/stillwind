import assert from "node:assert/strict";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { NativeDesignSystemLoader } from "@stillwind/native";

import { EditorProject } from "../src/editor.ts";

const root = path.dirname(fileURLToPath(import.meta.url));
const entry = path.join(root, "document.tsx");
const dependency = path.join(root, "tokens.ts");
const css =
  "@utility red { color: red; } @utility blue { color: blue; } @stillwind-slot utilities;";
const literal = "import { style } from 'stillwind/react'; export const props = style('red');";
const imported =
  "import { style } from 'stillwind/react'; import { token } from './tokens'; export const props = style(token(''));";
function project(
  read: (id: string) => Promise<string> = async () => {
    throw new Error("Unexpected disk read");
  },
) {
  return new EditorProject(
    root,
    {
      resolve: async (specifier, importer) => ({
        id: path.resolve(path.dirname(importer), `${specifier}.ts`),
      }),
      read,
      watch: () => {},
      warn: () => {},
    },
    () => {
      const loader = new NativeDesignSystemLoader('{"classIdentity":"module"}');
      loader.provide(0, path.join(root, "root.css"), css);
      return loader;
    },
  );
}

void test("unsaved source, broken syntax, recovery and macro removal retain explicit versions", async () => {
  const editor = project();
  editor.update({ id: entry, version: 1, source: literal });
  const first = await editor.analyze(entry);
  assert.equal(first.status, "success");
  assert.match(first.output!.css, /color:\s*red/);
  editor.update({ id: entry, version: 2, source: literal + " const broken = (" });
  const broken = await editor.analyze(entry);
  assert.equal(broken.status, "partial");
  assert.equal(broken.previousSuccess?.version, 1);
  assert.ok(broken.error || broken.output?.diagnostics.length);
  editor.update({ id: entry, version: 3, source: "export const props = {};" });
  const removed = await editor.analyze(entry);
  assert.equal(removed.status, "success");
  assert.equal(removed.output?.css, "");
  assert.throws(() => editor.update({ id: entry, version: 3, source: literal }), /monotonically/);
  editor.close(entry);
  await assert.rejects(editor.analyze(entry), /not open/);
});

void test("dependency overlays are used and removing an import stops dependency reads", async () => {
  const editor = project();
  editor.update({
    id: dependency,
    version: 1,
    source:
      "import { style } from 'stillwind/react'; export const token = style.animation({ from: 'red', to: 'red' });",
  });
  editor.update({ id: entry, version: 1, source: imported });
  const first = await editor.analyze(entry);
  assert.equal(first.status, "success", first.error ?? JSON.stringify(first.output?.diagnostics));
  assert.match(first.output!.css, /color:\s*red/);
  assert.deepEqual(first.dependencies, [dependency]);
  editor.update({
    id: dependency,
    version: 2,
    source:
      "import { style } from 'stillwind/react'; export const token = style.animation({ from: 'blue', to: 'blue' });",
  });
  const next = await editor.analyze(entry);
  assert.equal(next.status, "success");
  assert.match(next.output!.css, /color:\s*blue/);
  editor.update({ id: entry, version: 2, source: literal });
  assert.deepEqual((await editor.analyze(entry)).dependencies, []);
});

void test("late dependency reads and cancellation cannot publish or replace the last success", async () => {
  let release!: (source: string) => void;
  let started!: () => void;
  const reading = new Promise<void>((resolve) => {
    started = resolve;
  });
  const editor = project(async () => {
    started();
    return new Promise<string>((resolve) => {
      release = resolve;
    });
  });
  editor.update({ id: entry, version: 1, source: imported });
  const old = editor.analyze(entry);
  await reading;
  editor.update({ id: entry, version: 2, source: literal });
  assert.equal((await editor.analyze(entry)).status, "success");
  release(
    "import { style } from 'stillwind/react'; export const token = style.animation({ from: 'blue', to: 'blue' });",
  );
  const stale = await old;
  assert.equal(stale.status, "stale");
  assert.equal(stale.output, undefined);
  const cancelled = await editor.analyze(entry, AbortSignal.abort());
  assert.equal(cancelled.status, "cancelled");
  assert.equal(cancelled.previousSuccess?.version, 2);
});

void test("semantic reads select a barrel export and exclude unrelated runtime imports", async () => {
  const reads: string[] = [];
  const barrel = path.join(root, "barrel.ts");
  const editor = project(async (id) => {
    reads.push(id);
    if (id === barrel)
      return "export { token } from './tokens'; export { lazy } from './unvisited';";
    if (id === dependency)
      return "import { style } from 'stillwind/react'; export const token = style.animation({ from: 'red', to: 'red' });";
    throw new Error(`Unrelated source was read: ${id}`);
  });
  editor.update({
    id: entry,
    version: 1,
    source:
      "import { style } from 'stillwind/react'; import { token } from './barrel'; import { route } from './unvisited'; const unused = route; const alias = token; export const props = style(alias(''));",
  });
  const result = await editor.analyze(entry);
  assert.equal(
    result.status,
    "success",
    result.error ?? JSON.stringify(result.output?.diagnostics),
  );
  assert.match(result.output!.css, /color:\s*red/);
  assert.deepEqual(reads, [barrel, dependency]);
  assert.ok(result.checkerStats!.overlayReads > 0);
  assert.ok(result.checkerStats!.diskReadRequests > 0);
});
