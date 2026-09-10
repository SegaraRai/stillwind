import assert from "node:assert/strict";
import { readFileSync, writeFileSync } from "node:fs";

const read = (name) =>
  JSON.parse(readFileSync(new URL(`./results/${name}.json`, import.meta.url), "utf8"));
const indexes = process.argv
  .slice(2)
  .filter((value) => /^\d+$/.test(value))
  .map(Number);
if (!indexes.length) indexes.push(5, 6, 7);
const current = indexes.map((index) => read(`current-${index}`));
const legacy = indexes.map((index) => read(`legacy-${index}`));
for (const run of [...current, ...legacy]) {
  assert.deepEqual(
    run.fingerprints,
    current[0].fingerprints,
    "Generated JS/CSS must match before comparison",
  );
  assert.equal(run.fullCssSha256, current[0].fullCssSha256);
}
const stats = (values) => {
  const sorted = [...values].sort((a, b) => a - b);
  return {
    medianMs: sorted[Math.floor(sorted.length / 2)],
    minMs: sorted[0],
    maxMs: sorted.at(-1),
  };
};
const rows = current[0].rows.map(({ name }) => {
  const value = (runs) =>
    stats(runs.map((run) => run.rows.find((row) => row.name === name).medianMs));
  const old = value(legacy);
  const next = value(current);
  return { name, current: next, legacy: old, ratio: next.medianMs / old.medianMs };
});
const summary = {
  runs: indexes,
  metric:
    "Median of independent process medians; min/max are process medians, not confidence intervals.",
  correctness:
    "All six generated JS/CSS fingerprints and full registry CSS hashes match across selected current and baseline processes.",
  currentBindings: [...new Set(current.map((run) => run.bindingSha256))],
  legacyBindings: [...new Set(legacy.map((run) => run.bindingSha256))],
  rows,
};
writeFileSync(
  new URL("./results/summary.json", import.meta.url),
  JSON.stringify(summary, null, 2) + "\n",
);
for (const row of rows)
  console.log(
    `${row.name}: current ${row.current.medianMs.toFixed(3)} [${row.current.minMs.toFixed(3)}, ${row.current.maxMs.toFixed(3)}]; old ${row.legacy.medianMs.toFixed(3)}; ratio ${row.ratio.toFixed(2)}`,
  );
