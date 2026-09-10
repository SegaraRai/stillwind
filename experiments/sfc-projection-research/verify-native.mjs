import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";

const report = JSON.parse(readFileSync("results/research.json", "utf8"));
const rows = report.rows.filter((row) =>
  ["astro-tsx", "svelte2tsx", "vue-language-core"].includes(row.adapter),
);
const inputs = rows.map((row) => {
  const base = `results/outputs/${row.adapter}-${row.fixture}`;
  const rawMap = JSON.parse(readFileSync(`${base}.map.json`, "utf8"));
  return {
    name: `${row.adapter}/${row.fixture}`,
    generated: readFileSync(
      `${base}${row.adapter === "vue-language-core" ? ".ts" : ".tsx"}`,
      "utf8",
    ),
    original: readFileSync(`fixtures/${row.language}-${row.fixture}.${row.language}`, "utf8"),
    map:
      row.mapKind === "v3"
        ? { kind: "v3", json: JSON.stringify(rawMap), source: rawMap.sources[0] }
        : { kind: "volar", mappings: rawMap },
    probes: row.probes.map((probe) => [
      probe.candidates[0].generatedStart,
      probe.candidates[0].generatedEnd,
    ]),
  };
});
const results = JSON.parse(
  execFileSync(
    "cargo",
    ["run", "--quiet", "-p", "stillwind-source", "--example", "projection_probe"],
    {
      input: JSON.stringify(inputs),
      encoding: "utf8",
      maxBuffer: 16 * 1024 * 1024,
      env: { ...process.env, CARGO_INCREMENTAL: "0" },
    },
  ),
);
let accepted = 0;
const failures = [];
for (const [index, result] of results.entries()) {
  if (result.error) {
    failures.push({ name: result.name, error: result.error });
    continue;
  }
  for (const [probeIndex, probe] of result.probes.entries()) {
    const expected = rows[index].probes[probeIndex];
    if (probe.error) failures.push({ name: result.name, raw: expected.raw, error: probe.error });
    else {
      assert.deepEqual(probe.range, [expected.start, expected.end]);
      accepted++;
    }
  }
}
writeFileSync(
  "results/native-verification.json",
  JSON.stringify({ accepted, failures, results }, null, 2) + "\n",
);
console.log(JSON.stringify({ accepted, failures }, null, 2));
assert.equal(accepted, 77);
assert.equal(failures.length, 7);
assert(
  failures.every((failure) => failure.name.startsWith("vue-language-core/") && failure.raw),
  "Only the seven projected Vue template expressions may be rejected before normalization.",
);
