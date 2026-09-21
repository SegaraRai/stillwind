import { readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";

import { compileSource } from "@stillwind/native";

import { parseMarkdownFixtureCases } from "./lib/markdown-fixtures.ts";
import { stillwindFixtureCss } from "./lib/stillwind-fixture-css.ts";
import { compileTailwindCss } from "./lib/tailwind-compiler.ts";

const root = resolve(import.meta.dirname, "..");
const fixtureRoot = join(root, "fixtures", "tailwind-v4");
const caseRoot = join(fixtureRoot, "cases");
const themePath = join(fixtureRoot, "numeric-domain-audit.css");

function utilityStart(candidate: string): number {
  let start = 0;
  let brackets = 0;
  let parentheses = 0;
  let escaped = false;
  for (let index = 0; index < candidate.length; index += 1) {
    const character = candidate[index];
    if (escaped) escaped = false;
    else if (character === "\\") escaped = true;
    else if (character === "[") brackets += 1;
    else if (character === "]") brackets -= 1;
    else if (character === "(") parentheses += 1;
    else if (character === ")") parentheses -= 1;
    else if (character === ":" && brackets === 0 && parentheses === 0) start = index + 1;
  }
  return start;
}

function numericMutations(candidate: string): string[] {
  const start = utilityStart(candidate);
  const prefix = candidate.slice(0, start);
  const utility = candidate.slice(start);
  const segments = [...utility.matchAll(/(^|[-/])(\d+(?:\.\d+)?)(%?)(?=$|[-/!])/g)];
  return segments.flatMap((segment, ordinal) => {
    const numberStart = (segment.index ?? 0) + segment[1].length;
    const numberEnd = numberStart + segment[2].length;
    return [37, 113].map((probe) => {
      const value = probe + ordinal * 2;
      return `${prefix}${utility.slice(0, numberStart)}${value}${utility.slice(numberEnd)}`;
    });
  });
}

function hasGeneratedUtility(css: string, baseline: string): boolean {
  return css.replaceAll(/\/\*[\s\S]*?\*\//g, "").trim() !== baseline;
}

const failures: string[] = [];
const checked = new Set<string>();
let acceptedByTailwind = 0;

for (const file of readdirSync(caseRoot)
  .filter((name) => name.endsWith(".fixture.md"))
  .sort()) {
  const markdown = readFileSync(join(caseRoot, file), "utf8");
  for (const fixtureCase of parseMarkdownFixtureCases(markdown)) {
    if (/@(?:source|config|plugin|reference)\b/.test(fixtureCase.input)) continue;
    const baseline = (
      await compileTailwindCss({
        css: fixtureCase.input,
        candidates: [],
        base: fixtureRoot,
        from: themePath,
      })
    )
      .replaceAll(/\/\*[\s\S]*?\*\//g, "")
      .trim();
    for (const original of fixtureCase.candidates.split(/\s+/).filter(Boolean)) {
      for (const candidate of numericMutations(original)) {
        const key = `${fixtureCase.input}\0${candidate}`;
        if (checked.has(key)) continue;
        checked.add(key);
        const tailwind = await compileTailwindCss({
          css: fixtureCase.input,
          candidates: [candidate],
          base: fixtureRoot,
          from: themePath,
        });
        if (!hasGeneratedUtility(tailwind, baseline)) continue;
        acceptedByTailwind += 1;
        const source = `import { style } from "stillwind/react"; style(${JSON.stringify(candidate)});`;
        const result = compileSource(
          source,
          "src/NumericDomainAudit.tsx",
          stillwindFixtureCss(fixtureCase.input),
        ) as { classes: unknown[]; diagnostics: unknown[] };
        if (result.classes.length === 0 || result.diagnostics.length !== 0) {
          failures.push(`${file}: ${original} -> ${candidate}`);
        }
      }
    }
  }
}

if (failures.length > 0) {
  throw new Error(
    `Numeric utility domain gaps:\n${failures.map((item) => `- ${item}`).join("\n")}`,
  );
}

console.log(
  `Verified ${acceptedByTailwind} Tailwind-accepted numeric mutations across ${checked.size} probes.`,
);
