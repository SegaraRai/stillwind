import assert from "node:assert/strict";
import { mkdirSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { cssWithSourceMap, decodeCssSourceMap } from "../../packages/host/src/css-source-map.ts";
import { cssWithSourceMap as candidate } from "./candidate.ts";
import { cssInput } from "./input.ts";

const rows: { name: string; rules: number; medianMs: number; samplesMs: number[] }[] = [];
for (const rules of [10, 100, 1000, 10_000]) {
  const { output, replacements } = cssInput(rules);
  assert.deepEqual(
    decodeCssSourceMap(candidate(output, replacements)),
    decodeCssSourceMap(cssWithSourceMap(output, replacements)),
  );
  for (const [name, implementation] of [
    ["production", cssWithSourceMap],
    ["candidate", candidate],
  ] as const) {
    implementation(output, replacements);
    const samplesMs: number[] = [];
    for (let i = 0; i < 5; i++) {
      const start = performance.now();
      implementation(output, replacements);
      samplesMs.push(performance.now() - start);
    }
    const medianMs = [...samplesMs].sort((a, b) => a - b)[2];
    rows.push({ name, rules, medianMs, samplesMs });
    console.log(`${name}/${rules}: ${medianMs.toFixed(3)} ms`);
  }
}
const directory = fileURLToPath(new URL("../results/", import.meta.url));
mkdirSync(directory, { recursive: true });
writeFileSync(
  `${directory}/source-map.json`,
  JSON.stringify({ timestamp: new Date().toISOString(), node: process.version, rows }, null, 2) +
    "\n",
);
