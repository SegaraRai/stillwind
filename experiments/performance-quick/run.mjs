import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import os from "node:os";
import path from "node:path";
import { performance } from "node:perf_hooks";
import { fileURLToPath } from "node:url";

import * as current from "../../packages/native/dist/index.js";
import { resolveFixtureCss } from "../../tools/lib/stillwind-fixture-css.ts";

const args = process.argv.slice(2);
const argument = (name, fallback) => {
  const index = args.indexOf(name);
  return index < 0 ? fallback : args[index + 1];
};
const legacyPath = argument("--legacy", null);
const native = legacyPath ? createRequire(import.meta.url)(path.resolve(legacyPath)) : current;
const label = argument("--label", legacyPath ? "legacy" : "current");
const directory = fileURLToPath(new URL("./results/", import.meta.url));
const input = (value) => (legacyPath ? JSON.stringify(value) : value);
const output = (value) => (legacyPath ? JSON.parse(value) : value);
const options = {};
const minimal =
  "@utility flex {display:flex;} @utility block {display:block;} @utility red {color:red;} @stillwind-slot utilities;";
const full = resolveFixtureCss('@import "stillwind";');
const rows = [];
const fingerprints = {};
const hash = (value) => createHash("sha256").update(value).digest("hex");
let sink;
const quantile = (values, q) => [...values].sort((a, b) => a - b)[Math.ceil(values.length * q) - 1];
function measure(name, operation, iterations = 9, warmup = 2) {
  for (let i = 0; i < warmup; i++) {
    sink = operation();
  }
  const samples = [];
  for (let i = 0; i < iterations; i++) {
    const start = performance.now();
    sink = operation();
    samples.push(performance.now() - start);
    if (samples.reduce((a, b) => a + b, 0) > 30_000) {
      break;
    }
  }
  const row = {
    name,
    medianMs: quantile(samples, 0.5),
    p90Ms: quantile(samples, 0.9),
    samplesMs: samples,
  };
  rows.push(row);
  console.log(`${label}: ${name}: ${row.medianMs.toFixed(3)} ms (n=${samples.length})`);
  return sink;
}
function loader(css) {
  const value = new native.NativeDesignSystemLoader(input(options));
  output(value.provide(0, "root.css", css));
  return value;
}
function compile(session, sources) {
  return output(session.compile(input(sources)));
}
function sources(count, calls = 8) {
  return Array.from({ length: count }, (_, module) => ({
    path: `module-${module}.tsx`,
    source:
      "import {style} from 'stillwind/react';\n" +
      Array.from({ length: calls }, (_, i) => `export const props${i}=style('flex red');`).join(
        "\n",
      ),
  }));
}
const minimalLoader = measure("loader/minimal", () => loader(minimal));
const fullLoader = measure("loader/full-registry", () => loader(full));
const representative = sources(1)[0];
for (const [name, css] of [
  ["minimal", minimal],
  ["full-registry", full],
]) {
  const compiled = measure(`stateless/${name}/8-calls`, () =>
    output(native.compileSource(representative.source, representative.path, css, input(options))),
  );
  assert.equal(compiled.diagnostics.length, 0);
  assert.ok(compiled.code.includes("className"));
  fingerprints[`stateless/${name}`] = hash(JSON.stringify([compiled.code, compiled.css]));
}
for (const calls of [1, 100, 1000]) {
  const data = sources(1, calls);
  const session = new native.NativeProjectSession(minimalLoader, true);
  const first = compile(session, data);
  assert.equal(first.modules[0].diagnostics.length, 0);
  assert.ok(first.modules[0].mappings.length >= calls);
  measure(`session/unchanged/${calls}-calls`, () => compile(session, data));
  measure(`summary/${calls}-calls`, () =>
    output(native.summarizeSource(data[0].source, data[0].path)),
  );
}
for (const count of [10, 100, 300, 1000]) {
  const data = sources(count);
  const first = measure(
    `graph/fresh/${count}-modules`,
    () => compile(new native.NativeProjectSession(minimalLoader, true), data),
    5,
    1,
  );
  assert.equal(first.modules.length, count);
  assert.ok(first.modules.every((module) => module.diagnostics.length === 0));
  fingerprints[`graph/${count}`] = hash(
    JSON.stringify(first.modules.map(({ path, code, css }) => ({ path, code, css }))),
  );
  const session = new native.NativeProjectSession(minimalLoader, true);
  compile(session, data);
  measure(`graph/unchanged/${count}-modules`, () => compile(session, data));
  let edit = 0;
  measure(`graph/single-edit/${count}-resident-modules`, () =>
    compile(session, [
      { ...data[0], source: data[0].source.replaceAll("flex", ++edit % 2 ? "block" : "flex") },
    ]),
  );
  measure(`css/all/${count}-modules`, () => session.css());
  const selection = {
    modules: data.map((source) => source.path),
    coverage: { kind: "observed" },
    placement: "document",
  };
  measure(`css/plan-with-map/${count}-modules`, () =>
    output(session.planStylesheet(input(selection))),
  );
  measure(`css/single-carrier/${count}-resident-modules`, () => session.moduleCss(data[0].path));
}
const fullSession = new native.NativeProjectSession(fullLoader, true);
compile(fullSession, [representative]);
measure("css/full-registry/base-with-map", () => output(fullSession.baseCssWithMap()));
measure("css/full-registry/single-carrier", () => fullSession.moduleCss(representative.path));
mkdirSync(directory, { recursive: true });
const result = {
  timestamp: new Date().toISOString(),
  label,
  legacyPath,
  revision: execFileSync("git", ["rev-parse", "HEAD"], { encoding: "utf8" }).trim(),
  baselineRevision: legacyPath ? "9f4e668" : null,
  node: process.version,
  platform: `${process.platform}-${process.arch}`,
  cpu: os.cpus()[0].model,
  bindingSha256: hash(readFileSync(legacyPath ?? current.nativeBindingPath)),
  fingerprints,
  cpuCount: os.cpus().length,
  memory: process.memoryUsage(),
  fullCssBytes: Buffer.byteLength(full),
  fullCssSha256: createHash("sha256").update(full).digest("hex"),
  notes:
    "Release native binary; in-process timings exclude Node startup and filesystem CSS reads. Legacy includes JSON.stringify/parse. No app build or HMR timing. Warmup=2 (fresh graphs=1); n=9 (fresh graphs=5), capped at 30s measured per case. Correctness assertions outside timed region.",
  rows,
};
writeFileSync(path.join(directory, `${label}.json`), JSON.stringify(result, null, 2) + "\n");
