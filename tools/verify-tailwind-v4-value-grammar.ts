import { join, resolve } from "node:path";
import { compileTailwindCss } from "./lib/tailwind-compiler.ts";
import { readValueGrammarCases } from "./lib/value-grammar.ts";

const root = resolve(import.meta.dirname, "..");
const fixtureRoot = join(root, "fixtures", "tailwind-v4");
const input = {
  css: '@import "tailwindcss/theme"; @import "tailwindcss/utilities";',
  base: fixtureRoot,
};
const baseline = await compileTailwindCss({ ...input, candidates: [] });
const cases = readValueGrammarCases(join(fixtureRoot, "catalog", "value-grammar.md"));

for (const { candidate, accepted } of cases) {
  const output = await compileTailwindCss({ ...input, candidates: [candidate] });
  const actual = output !== baseline;
  if (actual !== accepted) {
    throw new Error(
      `Tailwind value grammar drift for ${candidate}: expected ${accepted ? "accepted" : "rejected"}, got ${actual ? "accepted" : "rejected"}`,
    );
  }
}

console.log(`Verified ${cases.length} Tailwind value grammar cases.`);
