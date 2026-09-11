import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";

const option = (name) => process.argv[process.argv.indexOf(name) + 1];
const binding = option("--binding");
assert.ok(process.argv.includes("--binding"), "Pass --binding <instrumented.node>");
if (process.argv.includes("--worker")) {
  const native = createRequire(import.meta.url)(path.resolve(binding));
  const count = Number(option("--count"));
  const loader = new native.NativeDesignSystemLoader({});
  loader.provide(
    0,
    "root.css",
    "@utility flex {display:flex;} @utility red {color:red;} @stillwind-slot utilities;",
  );
  const session = new native.NativeProjectSession(loader, true);
  const sources = Array.from({ length: count }, (_, module) => ({
    path: `module-${module}.tsx`,
    source:
      "import {style} from 'stillwind/react';\n" +
      Array.from({ length: 8 }, (_, i) => `export const props${i}=style('flex red');`).join("\n"),
  }));
  const start = performance.now();
  const output = session.compile(sources);
  const elapsedMs = performance.now() - start;
  assert.equal(output.modules.length, count);
  assert.ok(output.modules.every((module) => module.diagnostics.length === 0));
  console.log(JSON.stringify({ count, elapsedMs }));
} else {
  const directory = fileURLToPath(new URL("./results/", import.meta.url));
  mkdirSync(directory, { recursive: true });
  const rows = [];
  for (const count of [100, 300, 1000]) {
    const child = spawnSync(
      process.execPath,
      [fileURLToPath(import.meta.url), "--worker", "--binding", binding, "--count", String(count)],
      { encoding: "utf8", maxBuffer: 10 * 1024 * 1024, timeout: 60_000 },
    );
    assert.equal(child.status, 0, child.stderr);
    const spans = child.stderr
      .split(/\r?\n/)
      .filter((line) => line.startsWith("SWPERF "))
      .map((line) => line.split(" ").slice(1).map(Number));
    assert.equal(spans.length, count);
    const names = [
      "oldCssMs",
      "backendCompileMs",
      "symbolCheckAndStoreMs",
      "reindexMs",
      "newCssMs",
    ];
    const totals = Object.fromEntries(
      names.map((name, i) => [name, spans.reduce((sum, row) => sum + row[i], 0) / 1e6]),
    );
    const row = { ...JSON.parse(child.stdout), ...totals };
    rows.push(row);
    writeFileSync(path.join(directory, `profile-${count}.log`), child.stderr);
    console.log(row);
  }
  const checkout = path.dirname(path.resolve(binding));
  writeFileSync(
    fileURLToPath(new URL("./instrumentation.patch", import.meta.url)),
    execFileSync("git", ["-C", checkout, "diff", "--", "crates/project/src/session.rs"]),
  );
  writeFileSync(
    path.join(directory, "profile.json"),
    JSON.stringify(
      {
        timestamp: new Date().toISOString(),
        revision: execFileSync("git", ["-C", checkout, "rev-parse", "HEAD"], {
          encoding: "utf8",
        }).trim(),
        notes:
          "One fresh graph per size in separate processes. Instrumented temporary checkout; five timed spans in update_module, each recorded in stderr. Total includes untimed scan/finalize/output/FFI and instrumentation overhead. No optimized behavior changes.",
        rows,
      },
      null,
      2,
    ) + "\n",
  );
}
