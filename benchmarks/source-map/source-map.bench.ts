import { expect, test, type BenchRunOptions } from "vite-plus/test";

import { cssWithSourceMap, decodeCssSourceMap } from "../../packages/host/src/css-source-map.ts";
import { cssWithSourceMap as candidate } from "./candidate.ts";
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
    let productionResult!: string;
    let candidateResult!: string;
    await bench.compare(
      bench(`production/${rules}`, { async: false }, () => {
        productionResult = cssWithSourceMap(output, replacements);
      }),
      bench(`candidate/${rules}`, { async: false }, () => {
        candidateResult = candidate(output, replacements);
      }),
      options,
    );
    expect(decodeCssSourceMap(candidateResult)).toStrictEqual(decodeCssSourceMap(productionResult));
  });
}
