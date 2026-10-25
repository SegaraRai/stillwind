import { spawnSync } from "node:child_process";
import path from "node:path";

const root = path.resolve(import.meta.dirname, "../..");
const directory = path.resolve(root, process.argv[2] ?? "apps/website");
const iterations = Number(process.argv[3] ?? 3);
if (!Number.isSafeInteger(iterations) || iterations < 1) {
  throw new Error("Iterations must be a positive integer");
}
const measurements = [];
for (const operation of ["lint", "fmt"] as const) {
  const samples = [];
  let summary: unknown;
  for (let iteration = 0; iteration < iterations; iteration++) {
    const start = performance.now();
    const result = spawnSync(
      process.execPath,
      [
        path.join(root, "packages/cli/dist/main.js"),
        operation,
        ...(operation === "fmt" ? ["--check"] : []),
        "--format",
        "json",
      ],
      { cwd: directory, encoding: "utf8", maxBuffer: 32 * 1024 * 1024, windowsHide: true },
    );
    samples.push(performance.now() - start);
    if (result.error || result.status === null || result.status > 1) {
      throw result.error ?? new Error(result.stderr || result.stdout);
    }
    summary = JSON.parse(result.stdout).summary;
  }
  measurements.push({ operation, samples, summary });
}
console.log(JSON.stringify({ directory, measurements }, null, 2));
