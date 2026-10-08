import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { cpus } from "node:os";
import { resolve } from "node:path";

import { resolveFixtureCss } from "../../tools/fixture-css/stillwind-css.ts";

const [bindingPath, outputPath, mode = "measure"] = process.argv.slice(2);
const { NativeDesignSystemLoader, NativeProjectSession } = createRequire(import.meta.url)(
  resolve(bindingPath),
);
const css = resolveFixtureCss(
  '@import "stillwind"; @theme { --spacing:.25rem; --color-brand:red; }',
);

const loader = () => {
  const current = new NativeDesignSystemLoader();
  current.provide(0, "theme.css", css, "untitled:theme.css");
  return current;
};
const source = (unique, separate) => {
  const candidates = Array.from(
    { length: 100 },
    (_, i) => `bg-[#${(0x123456 + (unique ? i : 0)).toString(16)}]`,
  );
  const calls = separate
    ? candidates.map((candidate, i) => `export const card${i}=style('${candidate}');`).join("\n")
    : `export const card=style('${candidates.join(" ")}');`;
  return {
    kind: "direct",
    moduleId: "entry.ts",
    document: {
      uri: "untitled:entry.ts",
      format: "script",
      code: `import {style} from 'stillwind/react'; ${calls}`,
    },
    syntax: { language: "ts", moduleKind: "module" },
    typeFacts: [],
  };
};
const hash = (value) => createHash("sha256").update(JSON.stringify(value)).digest("hex");
const cases = [];
async function measure(name, before, operation, validate, inputHash) {
  const samples = [];
  const repetitions = mode === "profile" ? 2 : 10;
  let contract;
  for (let i = 0; i < repetitions; i++) {
    if (global.gc) {
      global.gc();
      await new Promise(setImmediate);
    }
    const input = before();
    console.error(`CASE ${name} ${i}`);
    const start = performance.now();
    const result = operation(input);
    const elapsed = performance.now() - start;
    const checked = validate(result);
    if (contract && contract !== checked) {
      throw new Error(`Unstable output for ${name}`);
    }
    contract = checked;
    if (i > 0) {
      samples.push(elapsed);
    }
  }
  const ordered = [...samples].sort((a, b) => a - b);
  cases.push({
    name,
    samples,
    medianMs: ordered[Math.floor(ordered.length / 2)],
    contract,
    inputHash,
  });
}
await measure(
  "prepare-css-revision",
  () => null,
  loader,
  (value) => {
    const query = value.editorQuery();
    if (query.canonicalCandidate("p-[16px]") !== "p-4") {
      throw new Error("Canonical spacing mismatch");
    }
    return hash(query.canonicalCandidate("bg-[#f00]"));
  },
  hash(css),
);
const shared = loader();
for (const [name, candidate, expected] of [
  ["spacing", "p-[16px]", "p-4"],
  ["color-match", "bg-[#f00]", "bg-brand"],
  ["color-miss", "bg-[#123456]", null],
]) {
  await measure(
    `cold-query/${name}`,
    () => shared.editorQuery(),
    (query) => query.canonicalCandidate(candidate),
    (result) => {
      if (result !== expected) {
        throw new Error(`Wrong result for ${candidate}`);
      }
      return hash(result);
    },
    hash(candidate),
  );
}
for (const separate of [false, true]) {
  for (const unique of [false, true]) {
    const input = source(unique, separate);
    await measure(
      `module/${separate ? "independent" : "pairwise"}/${unique ? "unique" : "repeated"}/100`,
      () => new NativeProjectSession(shared, false),
      (session) => session.diagnose([input]),
      (result) => {
        const reports = result[0].reports;
        const expected = separate ? 0 : 4950;
        const code = unique ? "conflicting-utilities" : "duplicate-utilities";
        if (reports.length !== expected || reports.some((report) => report.code !== code)) {
          throw new Error(`Wrong report contract: ${reports.length}`);
        }
        return hash(result);
      },
      hash(input),
    );
  }
}
writeFileSync(
  outputPath,
  JSON.stringify(
    {
      node: process.version,
      cpu: cpus()[0].model,
      arch: process.arch,
      platform: process.platform,
      bindingSha256: createHash("sha256").update(readFileSync(bindingPath)).digest("hex"),
      cssSha256: hash(css),
      samples: mode === "profile" ? 1 : 9,
      warmup: 1,
      gcBetweenSamples: Boolean(global.gc),
      cases,
    },
    null,
    2,
  ) + "\n",
);
