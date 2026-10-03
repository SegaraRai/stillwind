import { join, resolve } from "node:path";
import { parseArgs } from "node:util";

import { compileTailwindCss } from "./compiler.ts";
import { verifyMarkdownFixtures } from "./markdown-fixtures.ts";

const root = resolve(import.meta.dirname, "../..");
const fixtureRoot = join(root, "fixtures", "tailwind-v4");
const {
  values: { check },
} = parseArgs({
  options: { check: { type: "boolean", default: false } },
});

const result = await verifyMarkdownFixtures({
  directory: join(fixtureRoot, "cases"),
  write: !check,
  compileCase: ({ input, candidates }) =>
    compileTailwindCss({
      css: input,
      candidates: candidates ? candidates.split(" ") : [],
      base: fixtureRoot,
    }),
});

if (result.changed && check) {
  process.exitCode = 1;
} else {
  console.log(
    `${check ? "Verified" : "Regenerated"} ${result.caseCount} Tailwind v4 fixture cases in ${result.fileCount} files.`,
  );
}
