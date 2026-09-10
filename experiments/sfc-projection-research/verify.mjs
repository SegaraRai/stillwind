import assert from "node:assert/strict";
import { readFileSync, writeFileSync } from "node:fs";

import { parseSync } from "oxc-parser";

import { adapters } from "./adapters.mjs";
import { fixtures } from "./fixtures.mjs";

const report = JSON.parse(readFileSync("results/research.json", "utf8"));
const outcomes = [];
for (const name of ["astro-tsx", "svelte2tsx", "vue-language-core"]) {
  const adapter = adapters.find((a) => a.name === name);
  for (const fixture of fixtures.filter((f) => f.language === adapter.language)) {
    const row = report.rows.find((r) => r.adapter === name && r.fixture === fixture.name);
    assert.equal(row.error, undefined);
    assert.deepEqual(row.parseErrors, []);
    const edits = [];
    for (const probe of row.probes) {
      const candidate = probe.candidates.find((c) => c.startAndContent && c.directBinding);
      if (name === "vue-language-core" && probe.region === "template") {
        assert.equal(candidate, undefined, "Vue template must not silently pass");
        continue;
      }
      assert.ok(candidate, `${name}/${fixture.name}: ${probe.raw}`);
      const start = candidate.mappedStarts.find((p) => p === probe.start);
      const end = start + candidate.text.length;
      assert.equal(fixture.source.slice(start, end), candidate.text);
      // Exercise both import-module replacement and expression replacement.
      const replacement =
        probe.kind === "import"
          ? probe.raw.replace(probe.module, "virtual:stillwind/research")
          : "{}";
      edits.push({ start, end, replacement });
    }
    edits.sort((a, b) => b.start - a.start);
    let source = fixture.source,
      boundary = source.length;
    for (const edit of edits) {
      assert.ok(edit.end <= boundary, "overlapping edits");
      source = source.slice(0, edit.start) + edit.replacement + source.slice(edit.end);
      boundary = edit.start;
    }
    const filename = `edited-${fixture.name}.${fixture.language}`;
    const converted = await adapter.convert(source, filename);
    assert.deepEqual(parseSync(filename + converted.extension, converted.code).errors, []);
    outcomes.push({ adapter: name, fixture: fixture.name, edits: edits.length, reconverted: true });
  }
}
writeFileSync("results/rewrite-verification.json", JSON.stringify(outcomes, null, 2) + "\n");
console.log(
  `Verified ${outcomes.length} edited SFCs, ${outcomes.reduce((n, r) => n + r.edits, 0)} mapped edits.`,
);
