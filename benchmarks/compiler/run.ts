import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { compileSource, NativeDesignSystemLoader, NativeProjectSession } from "@stillwind/native";
import type { NativePreparedSource, NativeStylesheetSelection } from "@stillwind/native";

import { resolveFixtureCss } from "../../tools/support/stillwind-css.ts";

const rows: { name: string; medianMs: number; samplesMs: number[] }[] = [];
const fingerprints: Record<string, string> = {};
function measure<T>(name: string, run: () => T, samples = 9): T {
  run();
  const samplesMs: number[] = [];
  let value!: T;
  for (let i = 0; i < samples; i++) {
    const start = performance.now();
    value = run();
    samplesMs.push(performance.now() - start);
    if (samplesMs.reduce((total, sample) => total + sample, 0) >= 30_000) {
      break;
    }
  }
  const medianMs = [...samplesMs].sort((a, b) => a - b)[Math.floor(samplesMs.length / 2)];
  rows.push({ name, medianMs, samplesMs });
  console.log(`${name}: ${medianMs.toFixed(3)} ms`);
  return value;
}
const minimal =
  "@utility flex {display:flex;} @utility block {display:block;} @utility red {color:red;} @stillwind-slot utilities;";
const full = resolveFixtureCss('@import "stillwind";');
const cssInput = (content: string) => ({
  content,
  name: "root.css",
  uri: "untitled:root.css",
  sourceMapSource: "untitled:root.css",
});
function loader(css: string) {
  const result = new NativeDesignSystemLoader();
  result.provide(0, "root.css", css, "untitled:root.css", "untitled:root.css");
  return result;
}
function sources(count: number, calls = 8): NativePreparedSource[] {
  return Array.from({ length: count }, (_, i) => ({
    kind: "direct",
    moduleId: `module-${i}.tsx`,
    document: {
      uri: `untitled:module-${i}.tsx`,
      sourceMapSource: `untitled:module-${i}.tsx`,
      format: "jsx",
      code:
        "import {style} from 'stillwind/react';\n" +
        Array.from({ length: calls }, (_, j) => `export const p${j}=style('flex red');`).join("\n"),
    },
    syntax: { language: "tsx", moduleKind: "unambiguous" },
  }));
}
for (const [name, css] of [
  ["minimal", minimal],
  ["full-registry", full],
] as const) {
  measure(`loader/${name}`, () => loader(css));
  const source = sources(1)[0];
  const output = measure(`standalone/${name}`, () =>
    compileSource(
      {
        source: source.document.code,
        moduleId: source.moduleId,
        uri: source.document.uri,
        sourceMapSource: source.document.sourceMapSource!,
        language: "tsx",
        moduleKind: "unambiguous",
        css: cssInput(css),
      },
      {},
    ),
  );
  assert.deepEqual(output.diagnostics, []);
}
const design = loader(minimal);
for (const calls of [1, 100, 1000]) {
  const input = sources(1, calls);
  const session = new NativeProjectSession(design, true);
  session.compile(input);
  measure(`unchanged/${calls}-calls`, () => session.compile(input));
}
for (const count of [10, 100, 300, 1000]) {
  const input = sources(count);
  const output = measure(
    `graph/fresh/${count}`,
    () => new NativeProjectSession(design, true).compile(input),
    5,
  );
  assert.equal(output.modules.length, count);
  assert.ok(output.modules.every((module) => module.diagnostics.length === 0));
  fingerprints[String(count)] = createHash("sha256")
    .update(JSON.stringify(output.modules))
    .digest("hex");
  const session = new NativeProjectSession(design, true);
  session.compile(input);
  measure(`graph/unchanged/${count}`, () => session.compile(input));
  let edit = 0;
  measure(`graph/single-edit/${count}`, () =>
    session.compile([
      {
        ...input[0],
        document: {
          ...input[0].document,
          code: input[0].document.code.replaceAll("flex", ++edit % 2 ? "block" : "flex"),
        },
      },
    ]),
  );
  const selection: NativeStylesheetSelection = {
    modules: input.map((source) => source.moduleId),
    coverage: { kind: "observed" },
    placement: "document",
  };
  measure(`css/plan/${count}`, () => session.planStylesheet(selection));
  measure(`css/carrier/${count}`, () => session.moduleCss(input[0].moduleId));
}
const directory = fileURLToPath(new URL("../results/", import.meta.url));
mkdirSync(directory, { recursive: true });
writeFileSync(
  `${directory}/compiler.json`,
  JSON.stringify(
    {
      timestamp: new Date().toISOString(),
      revision: execFileSync("git", ["rev-parse", "HEAD"], { encoding: "utf8" }).trim(),
      node: process.version,
      fingerprints,
      rows,
    },
    null,
    2,
  ) + "\n",
);
