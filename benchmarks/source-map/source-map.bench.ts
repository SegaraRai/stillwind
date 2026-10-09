import { createHash } from "node:crypto";

import { cssWithSourceMap, decodeCssSourceMap } from "@stillwind/host";
import { expect, test, type BenchRunOptions } from "vite-plus/test";

import { cssInput } from "./input.ts";

const options: BenchRunOptions = {
  iterations: 5,
  time: 0,
  warmupIterations: 1,
  warmupTime: 0,
};

for (const rules of [10, 100, 1000, 10_000]) {
  test(`${rules} rules`, async ({ bench, task }) => {
    const { output, replacements } = cssInput(rules);
    task.meta.measurement = options;
    task.meta.rules = rules;
    let result!: string;
    await bench(`production/${rules}`, { async: false }, () => {
      result = cssWithSourceMap(output, replacements);
    }).run(options);
    const decoded = decodeCssSourceMap(result);
    expect(decoded.code).not.toContain("ASSET_");
    expect(decoded.map?.sourcesContent).toStrictEqual([output.css]);
    task.meta.fingerprint = createHash("sha256").update(result).digest("hex");
  });
}
