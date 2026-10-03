import { cssWithSourceMap as candidate } from "../../benchmarks/source-map/candidate.ts";
import { cssInput } from "../../benchmarks/source-map/input.ts";
import { cssWithSourceMap as production } from "../../packages/host/src/css-source-map.ts";
const { output, replacements } = cssInput(10000);
const operation = process.argv[2] === "candidate" ? candidate : production;
for (let i = 0; i < 20; i++) {
  operation(output, replacements);
}
console.log(
  JSON.stringify({
    variant: process.argv[2],
    memory: process.memoryUsage(),
    resource: process.resourceUsage(),
  }),
);
