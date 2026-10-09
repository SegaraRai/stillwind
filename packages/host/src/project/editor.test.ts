import path from "node:path";
import { fileURLToPath } from "node:url";

import { NativeDesignSystemLoader } from "@stillwind/native";
import { expect, test } from "vite-plus/test";

import { fileModuleDescriptor } from "../compilation/index.ts";
import { EditorProject } from "./editor.ts";

const root = path.dirname(fileURLToPath(import.meta.url));
const entry = path.join(root, "document.tsx");
const dependency = path.join(root, "tokens.ts");
const css =
  "@utility red { color: red; } @utility blue { color: blue; } @stillwind-slot utilities;";
const literal = 'import { style } from "stillwind/react"; export const props = style("red");';
const imported =
  'import { style } from "stillwind/react"; import { token } from "./tokens"; export const props = style(token(""));';

/**
 * Creates an editor project with an in-memory theme and untitled diagnostic URIs.
 * @param read - Fallback source reader. The default fails if analysis attempts a disk read.
 * @returns A project whose relative imports resolve to sibling TypeScript paths.
 */
function project(
  read: (id: string) => Promise<string> = async () => {
    expect.fail("Unexpected disk read");
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
        document: {
          ...fileModuleDescriptor(root, id).document,
          uri: `untitled:${encodeURIComponent(id)}`,
        },
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

test("unsaved source, broken syntax, recovery and macro removal retain explicit versions", async () => {
  const editor = project();
  editor.update({ id: entry, version: 1, source: literal });
  const first = await editor.analyze(entry);
  expect(first.status).toBe("success");
  expect(first.output!.css).toMatch(/color:\s*red/);
  editor.update({
    id: entry,
    version: 2,
    source: literal + " const broken = (",
  });
  const broken = await editor.analyze(entry);
  expect(broken.status).toBe("partial");
  expect(broken.previousSuccess?.version).toBe(1);
  expect(broken.error || broken.output?.diagnostics.length).toBeTruthy();
  editor.update({ id: entry, version: 3, source: "export const props = {};" });
  const removed = await editor.analyze(entry);
  expect(removed.status).toBe("success");
  expect(removed.output?.css).toBe("");
  expect(() => editor.update({ id: entry, version: 3, source: literal })).toThrow(/monotonically/);
  editor.close(entry);
  await expect(editor.analyze(entry)).rejects.toThrow(/not open/);
});

test("component conversion failures retain reports and the previous successful preview", async () => {
  const id = path.join(root, "document.vue");
  const editor = project();
  const source =
    "<script setup>import {style} from 'stillwind/vue';const props=style('red');</script><template><div v-bind='props'/></template>";
  editor.update({ id, version: 1, source });
  expect((await editor.analyze(id)).status).toBe("success");
  editor.update({ id, version: 2, source: "<template><div></template>" });
  const failed = await editor.analyze(id);
  expect(failed.status).toBe("partial");
  expect(failed.error).toBeUndefined();
  expect(failed.output?.reports[0].code).toBe("SW01013");
  expect(failed.previousSuccess?.version).toBe(1);
  editor.update({ id, version: 3, source });
  const recovered = await editor.analyze(id);
  expect(recovered.status).toBe("success");
  expect(recovered.output?.reports).toEqual([]);
});

test("dependency overlays are used and removing an import stops dependency reads", async () => {
  const editor = project();
  editor.update({
    id: dependency,
    version: 1,
    source:
      'import { style } from "stillwind/react"; export const token = style.keyframes({ from: "red", to: "red" });',
  });
  editor.update({ id: entry, version: 1, source: imported });
  const first = await editor.analyze(entry);
  expect(first.status, first.error ?? JSON.stringify(first.output?.diagnostics)).toBe("success");
  expect(first.output!.css).toMatch(/color:\s*red/);
  expect(first.dependencies).toStrictEqual([dependency]);
  editor.update({
    id: dependency,
    version: 2,
    source:
      'import { style } from "stillwind/react"; export const token = style.keyframes({ from: "blue", to: "blue" });',
  });
  const next = await editor.analyze(entry);
  expect(next.status).toBe("success");
  expect(next.output!.css).toMatch(/color:\s*blue/);
  editor.update({ id: entry, version: 2, source: literal });
  expect((await editor.analyze(entry)).dependencies).toStrictEqual([]);
});

test("late dependency reads and cancellation cannot publish or replace the last success", async () => {
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
  expect((await editor.analyze(entry)).status).toBe("success");
  release(
    'import { style } from "stillwind/react"; export const token = style.keyframes({ from: "blue", to: "blue" });',
  );
  const stale = await old;
  expect(stale.status).toBe("stale");
  expect(stale.output).toBe(undefined);
  const cancelled = await editor.analyze(entry, AbortSignal.abort());
  expect(cancelled.status).toBe("cancelled");
  expect(cancelled.previousSuccess?.version).toBe(2);
});

test("semantic reads select a barrel export and exclude unrelated runtime imports", async () => {
  const reads: string[] = [];
  const barrel = path.join(root, "barrel.ts");
  const editor = project(async (id) => {
    reads.push(id);
    if (id === barrel) {
      return 'export { token } from "./tokens"; export { lazy } from "./unvisited";';
    }
    if (id === dependency) {
      return 'import { style } from "stillwind/react"; export const token = style.keyframes({ from: "red", to: "red" });';
    }
    expect.fail(`Unrelated source was read: ${id}`);
  });
  editor.update({
    id: entry,
    version: 1,
    source:
      'import { style } from "stillwind/react"; import { token } from "./barrel"; import { route } from "./unvisited"; const unused = route; const alias = token; export const props = style(alias(""));',
  });
  const result = await editor.analyze(entry);
  expect(result.status, result.error ?? JSON.stringify(result.output?.diagnostics)).toBe("success");
  expect(result.output!.css).toMatch(/color:\s*red/);
  expect(reads).toStrictEqual([barrel, dependency]);
  expect(result.checkerStats!.overlayReads).toBeGreaterThan(0);
  expect(result.checkerStats!.diskReadRequests).toBeGreaterThan(0);
});

test("default error contracts retain editable output and previous success", async () => {
  const editor = project();
  editor.update({ id: entry, version: 1, source: literal });
  expect((await editor.analyze(entry)).status).toBe("success");
  editor.update({
    id: entry,
    version: 2,
    source:
      'import { style } from "stillwind/react"; let n = 0; export const props = style({ "--x": n++ });',
  });
  const result = await editor.analyze(entry);
  expect(result.status).toBe("partial");
  expect(result.previousSuccess?.version).toBe(1);
  expect(result.output?.diagnostics.length).toBe(0);
  expect(result.output?.code).toBeTruthy();
  expect(result.output?.lintSettings["macro-argument-mutation"]).toBe("deny");
  expect(
    result.output?.lintFindings.some((finding) => finding.rule === "macro-argument-mutation"),
  ).toBeTruthy();
});

test("independent bases that share a property are reported", async () => {
  const editor = project();
  editor.update({
    id: entry,
    version: 1,
    source:
      'import { style } from "stillwind/react"; const a = style("red"); const b = style("blue"); export const props = style(a, b);',
  });
  const result = await editor.analyze(entry);
  expect(result.status, result.error ?? JSON.stringify(result.output?.diagnostics)).toBe("success");
  expect(result.output?.lintSettings["ambiguous-base-order"]).toBe("warn");
  expect(
    result.output?.lintFindings.some((finding) => finding.rule === "ambiguous-base-order"),
    JSON.stringify(result.output?.lintFindings),
  ).toBeTruthy();
});
