import { readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

import { compileTailwindCss } from "./lib/tailwind-compiler.ts";
import { isStillwindOwnedUtilityCandidate } from "./lib/stillwind-exclusions.ts";

const root = resolve(import.meta.dirname, "..");
const fixtureRoot = join(root, "fixtures", "tailwind-v4");
const catalogPath = join(fixtureRoot, "catalog", "default-completions.md");
const outputPath = join(root, "crates", "shorth-tailwind", "assets", "default-utilities.css");
const write = process.argv.includes("--write");
const catalog = readFileSync(catalogPath, "utf8");
const allCandidates = section(catalog, "Default completions").trim().split(/\s+/u);
const excluded = allCandidates.filter(isStillwindOwnedUtilityCandidate);
const candidates = allCandidates.filter(
  (candidate) => !isStillwindOwnedUtilityCandidate(candidate),
);
const input =
  '@import "tailwindcss/theme" layer(theme);\n@import "tailwindcss/utilities" layer(utilities);';
const rules = [];
const skipped = [];
const unsupportedAtRules = new Map();
const candidatesByAtRule = new Map();

for (const candidate of candidates) {
  const css = await compileTailwindCss({ css: input, candidates: [candidate], base: fixtureRoot });
  const body = firstUtilityBody(css);
  if (!body || /@(?!supports\b|media\b)/u.test(body)) {
    skipped.push(candidate);
    for (const match of body?.matchAll(/@(\w[\w-]*)/gu) ?? []) {
      if (match[1] !== "supports" && match[1] !== "media")
        unsupportedAtRules.set(match[1], (unsupportedAtRules.get(match[1]) ?? 0) + 1);
      if (match[1] !== "supports" && match[1] !== "media") {
        const entries = candidatesByAtRule.get(match[1]) ?? [];
        entries.push(candidate);
        candidatesByAtRule.set(match[1], entries);
      }
    }
    continue;
  }
  rules.push(`@utility ${candidate} {\n${indent(body.trim(), 2)}\n}`);
}

if (skipped.length > 0)
  console.warn(`Skipped ${skipped.length} completion rules with unsupported nested at-rules.`);
if (excluded.length > 0)
  console.warn(
    `Excluded ${excluded.length} Stillwind-owned completion rules: ${excluded.join(", ")}`,
  );
if (unsupportedAtRules.size > 0)
  console.warn(
    `Unsupported nested at-rules: ${[...unsupportedAtRules.entries()]
      .map(([name, count]) => `@${name} (${count})`)
      .join(", ")}`,
  );
for (const [name, entries] of candidatesByAtRule)
  console.warn(`@${name} candidates: ${entries.join(", ")}`);
const output = `/* GENERATED DEFAULT COMPLETIONS. Run vp run @stillwind/tools#registry:defaults:write. */\n${rules.join("\n")}\n`;
if (write) writeFileSync(outputPath, output);
console.log(
  `Lowered ${rules.length}/${candidates.length} eligible default completions into CSS registry.`,
);

function section(markdown, heading) {
  const expression = new RegExp(
    `## ${heading} \\(\\d+\\)\\n\\n\\\`\\\`\\\`text\\n([\\s\\S]*?)\\n\\\`\\\`\\\``,
  );
  const match = expression.exec(markdown);
  if (!match) throw new Error(`could not find ${heading} in ${catalogPath}`);
  return match[1];
}

function firstUtilityBody(css) {
  const layer = css.indexOf("@layer utilities {");
  if (layer === -1) return undefined;
  const selector = css.indexOf(".", layer);
  const open = css.indexOf("{", selector);
  if (selector === -1 || open === -1) return undefined;
  let depth = 1;
  for (let index = open + 1; index < css.length; index += 1) {
    if (css[index] === "{") depth += 1;
    if (css[index] === "}") depth -= 1;
    if (depth === 0) return css.slice(open + 1, index);
  }
  return undefined;
}

function indent(value, spaces) {
  const prefix = " ".repeat(spaces);
  return value
    .split("\n")
    .map((line) => `${prefix}${line.trim()}`)
    .join("\n");
}
