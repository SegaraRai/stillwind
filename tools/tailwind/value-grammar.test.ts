import { join, resolve } from "node:path";

import { expect, test } from "vite-plus/test";

import { compileTailwindCss } from "./compiler.ts";
import { readValueGrammarCases } from "./value-grammar.ts";

test("Tailwind accepts and rejects the catalogued value grammar", async () => {
  const root = resolve(import.meta.dirname, "../..");
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
    expect(actual, `Tailwind value grammar drift for ${candidate}`).toBe(accepted);
  }

  console.log(`Verified ${cases.length} Tailwind value grammar cases.`);
});
