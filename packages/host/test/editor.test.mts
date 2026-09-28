import assert from "node:assert/strict";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { NativeDesignSystemLoader } from "@stillwind/native";

import { fileModuleDescriptor } from "../src/compiler.ts";
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
      describe: (id) => ({
        ...fileModuleDescriptor(root, id),
        uri: `untitled:${encodeURIComponent(id)}`,
      }),
      watch: () => {},
      warn: () => {},
    },
    () => {
      const loader = new NativeDesignSystemLoader({});
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
      "import { style } from 'stillwind/react'; export const token = style.keyframes({ from: 'red', to: 'red' });",
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
      "import { style } from 'stillwind/react'; export const token = style.keyframes({ from: 'blue', to: 'blue' });",
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
    "import { style } from 'stillwind/react'; export const token = style.keyframes({ from: 'blue', to: 'blue' });",
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
    if (id === barrel) {
      return "export { token } from './tokens'; export { lazy } from './unvisited';";
    }
    if (id === dependency) {
      return "import { style } from 'stillwind/react'; export const token = style.keyframes({ from: 'red', to: 'red' });";
    }
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

void test("default error contracts retain editable output and previous success", async () => {
  const editor = project();
  editor.update({ id: entry, version: 1, source: literal });
  assert.equal((await editor.analyze(entry)).status, "success");
  editor.update({
    id: entry,
    version: 2,
    source:
      "import {style} from 'stillwind/react'; let n = 0; export const props = style({'--x': n++});",
  });
  const result = await editor.analyze(entry);
  assert.equal(result.status, "partial");
  assert.equal(result.previousSuccess?.version, 1);
  assert.equal(result.output?.diagnostics.length, 0);
  assert.ok(result.output?.code);
  assert.equal(result.output?.lint_policy["macro-argument-mutation"], "deny");
  assert.ok(
    result.output?.lint_findings.some((finding) => finding.rule === "macro-argument-mutation"),
  );
});

void test("independent bases that share a property are reported", async () => {
  const editor = project();
  editor.update({
    id: entry,
    version: 1,
    source:
      "import {style} from 'stillwind/react'; const a = style('red'); const b = style('blue'); export const props = style(a, b);",
  });
  const result = await editor.analyze(entry);
  assert.equal(
    result.status,
    "success",
    result.error ?? JSON.stringify(result.output?.diagnostics),
  );
  assert.equal(result.output?.lint_policy["ambiguous-base-order"], "warn");
  assert.ok(
    result.output?.lint_findings.some((finding) => finding.rule === "ambiguous-base-order"),
    JSON.stringify(result.output?.lint_findings),
  );
});
