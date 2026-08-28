import { join, resolve } from "node:path";
import { verifyMarkdownFixtures } from "./lib/markdown-fixtures.ts";
import { compileTailwindCss } from "./lib/tailwind-compiler.ts";

const root = resolve(import.meta.dirname, "..");
const fixtureRoot = join(root, "fixtures", "tailwind-v4");
const write = process.argv.includes("--write");

const result = await verifyMarkdownFixtures({
  directory: join(fixtureRoot, "cases"),
  write,
  compileCase: ({ input, candidates }) =>
    compileTailwindCss({
      css: input,
      candidates: candidates ? candidates.split(" ") : [],
      base: fixtureRoot,
    }),
});

if (result.changed && !write) {
  process.exitCode = 1;
} else {
  console.log(
    `${write ? "Regenerated" : "Verified"} ${result.caseCount} Tailwind v4 fixture cases in ${result.fileCount} files.`,
  );
}
