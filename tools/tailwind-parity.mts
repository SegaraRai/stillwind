import { readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";

import { compileReact, foldCalcExpressions } from "@stillwind/native";
import { transform } from "lightningcss";

import { parseMarkdownFixtureCases } from "./lib/markdown-fixtures.ts";
import { compileTailwindCss } from "./lib/tailwind-compiler.ts";

const root = resolve(import.meta.dirname, "..");
const fixtureRoot = join(root, "fixtures", "tailwind-v4");
const caseRoot = join(fixtureRoot, "cases");
const limit = Number(process.env.STILLWIND_PARITY_LIMIT ?? "0");
const offset = Number(process.env.STILLWIND_PARITY_OFFSET ?? "0");
const fileFilter = process.env.STILLWIND_PARITY_FILE;
const candidateFilter = process.env.STILLWIND_PARITY_CANDIDATE;
const debugRaw = process.env.STILLWIND_PARITY_DEBUG_RAW === "1";
const quiet = process.env.STILLWIND_PARITY_QUIET === "1" || process.argv.includes("--quiet");
const summaryOnly =
  process.env.STILLWIND_PARITY_SUMMARY_ONLY === "1" || process.argv.includes("--summary-only");
const analyze = process.env.STILLWIND_PARITY_ANALYZE === "1" || process.argv.includes("--analyze");

const categories = new Map<string, number>();
const files = new Map<string, number>();
const families = new Map<string, number>();
const missingProperties = new Map<string, number>();
const extraProperties = new Map<string, number>();
const missingRegistrations = new Map<string, number>();
const extraRegistrations = new Map<string, number>();
const declarationShapes = new Map<string, number>();
const categoryExamples = new Map<string, string[]>();
const categoryFamilies = new Map<string, Map<string, number>>();

function increment(counter: Map<string, number>, key: string): void {
  counter.set(key, (counter.get(key) ?? 0) + 1);
}

function topLevelSegments(css: string): string[] {
  const segments: string[] = [];
  let start = 0;
  let depth = 0;
  let quote = "";
  let escaped = false;
  for (let index = 0; index < css.length; index += 1) {
    const character = css[index];
    if (quote) {
      if (escaped) escaped = false;
      else if (character === "\\") escaped = true;
      else if (character === quote) quote = "";
      continue;
    }
    if (character === '"' || character === "'") {
      quote = character;
      continue;
    }
    if (character === "{") depth += 1;
    else if (character === "}") {
      depth -= 1;
      if (depth === 0) {
        segments.push(css.slice(start, index + 1));
        start = index + 1;
      }
    } else if (character === ";" && depth === 0) {
      segments.push(css.slice(start, index + 1));
      start = index + 1;
    }
  }
  if (start < css.length) segments.push(css.slice(start));
  return segments.filter(Boolean);
}

function withoutMetadata(css: string, includeTheme: boolean): string {
  return topLevelSegments(css)
    .filter(
      (segment) =>
        !segment.startsWith("@property ") &&
        !segment.startsWith("@layer properties") &&
        (!includeTheme || !segment.startsWith("@layer theme")),
    )
    .join("");
}

function declarationProperties(css: string): Set<string> {
  return new Set([...css.matchAll(/(?:^|[;{])((?:--)?[a-zA-Z][\w-]*):/g)].map((match) => match[1]));
}

function registeredProperties(css: string): Set<string> {
  return new Set([...css.matchAll(/@property\s+(--[a-zA-Z][\w-]*)/g)].map((match) => match[1]));
}

function isJustifiedPropertyExtension(expected: string, received: string): boolean {
  if (withoutMetadata(expected, false) !== withoutMetadata(received, false)) return false;
  const expectedRegistrations = registeredProperties(expected);
  const receivedRegistrations = registeredProperties(received);
  if ([...expectedRegistrations].some((property) => !receivedRegistrations.has(property))) {
    return false;
  }
  const additions = [...receivedRegistrations].filter(
    (property) => !expectedRegistrations.has(property),
  );
  return (
    additions.length > 0 &&
    additions.every(
      (property) => expected.includes(`var(${property}`) || expected.includes(`${property}:`),
    )
  );
}

function hasVariant(candidate: string): boolean {
  let brackets = 0;
  let parentheses = 0;
  let escaped = false;
  for (const character of candidate) {
    if (escaped) {
      escaped = false;
      continue;
    }
    if (character === "\\") escaped = true;
    else if (character === "[") brackets += 1;
    else if (character === "]") brackets -= 1;
    else if (character === "(") parentheses += 1;
    else if (character === ")") parentheses -= 1;
    else if (character === ":" && brackets === 0 && parentheses === 0) return true;
  }
  return false;
}

function candidateFamily(candidate: string): string {
  let utilityStart = 0;
  let brackets = 0;
  let parentheses = 0;
  let escaped = false;
  for (let index = 0; index < candidate.length; index += 1) {
    const character = candidate[index];
    if (escaped) {
      escaped = false;
      continue;
    }
    if (character === "\\") escaped = true;
    else if (character === "[") brackets += 1;
    else if (character === "]") brackets -= 1;
    else if (character === "(") parentheses += 1;
    else if (character === ")") parentheses -= 1;
    else if (character === ":" && brackets === 0 && parentheses === 0) utilityStart = index + 1;
  }
  const utility = candidate.slice(utilityStart);
  return utility
    .replaceAll(/\[[^\]]*\]/g, "[*]")
    .replaceAll(/\([^)]*\)/g, "(*)")
    .replaceAll(/-?\d+(?:\.\d+)?%?/g, "*")
    .replaceAll(/\/(?:[\w.-]+|\[[^\]]*\])/g, "/*");
}

function recordDifference(
  file: string,
  candidate: string,
  expected: string,
  received: string,
  forcedCategory?: string,
): void {
  if (!analyze) return;
  const withoutPropertiesExpected = withoutMetadata(expected, false);
  const withoutPropertiesReceived = withoutMetadata(received, false);
  const category =
    forcedCategory ??
    (expected === "" || expected === "@layer utilities;"
      ? "extra-output"
      : withoutPropertiesExpected === withoutPropertiesReceived
        ? "property-metadata-only"
        : withoutMetadata(expected, true) === withoutMetadata(received, true)
          ? "theme-metadata-only"
          : hasVariant(candidate)
            ? "variant-output"
            : "utility-output");
  increment(categories, category);
  increment(files, file);
  increment(families, candidateFamily(candidate));
  const familiesForCategory = categoryFamilies.get(category) ?? new Map<string, number>();
  increment(familiesForCategory, candidateFamily(candidate));
  categoryFamilies.set(category, familiesForCategory);
  const examples = categoryExamples.get(category) ?? [];
  if (examples.length < 5) examples.push(`${file} ${candidate}`);
  categoryExamples.set(category, examples);

  const expectedProperties = declarationProperties(withoutPropertiesExpected);
  const receivedProperties = declarationProperties(withoutPropertiesReceived);
  const missing = [...expectedProperties].filter((property) => !receivedProperties.has(property));
  const extra = [...receivedProperties].filter((property) => !expectedProperties.has(property));
  increment(
    declarationShapes,
    missing.length === 0
      ? extra.length === 0
        ? "same declaration names"
        : "extra declaration names"
      : extra.length === 0
        ? "missing declaration names"
        : "both missing and extra declaration names",
  );
  for (const property of expectedProperties) {
    if (!receivedProperties.has(property)) increment(missingProperties, property);
  }
  for (const property of receivedProperties) {
    if (!expectedProperties.has(property)) increment(extraProperties, property);
  }
  const expectedRegistrations = registeredProperties(expected);
  const receivedRegistrations = registeredProperties(received);
  for (const property of expectedRegistrations) {
    if (!receivedRegistrations.has(property)) increment(missingRegistrations, property);
  }
  for (const property of receivedRegistrations) {
    if (!expectedRegistrations.has(property)) increment(extraRegistrations, property);
  }
}

function printCounter(label: string, counter: Map<string, number>, limit = 20): void {
  console.log(`${label}:`);
  for (const [key, count] of [...counter]
    .sort((left, right) => right[1] - left[1])
    .slice(0, limit)) {
    console.log(`  ${count}\t${key}`);
  }
}

function minify(css: string): string {
  return Buffer.from(
    transform({
      filename: "parity.css",
      code: Buffer.from(foldCalcExpressions(css.replaceAll(/\/\*[\s\S]*?\*\//g, ""))),
      minify: true,
      errorRecovery: true,
    }).code,
  ).toString("utf8");
}

function selector(candidate: string): string {
  const escaped = candidate.replaceAll(/[^a-zA-Z0-9_-]/g, (value) =>
    value.codePointAt(0)! >= 0x80 ? value : `\\${value}`,
  );
  return /^\d/.test(candidate) ? `\\3${candidate[0]} ${escaped.slice(1)}` : escaped;
}

const failures: string[] = [];
let checked = 0;
let skipped = 0;
let unresolvedShown = 0;
for (const file of readdirSync(caseRoot)
  .filter((name) => name.endsWith(".fixture.md"))
  .filter((name) => !fileFilter || name === fileFilter)
  .sort()) {
  const markdown = readFileSync(join(caseRoot, file), "utf8");
  for (const fixtureCase of parseMarkdownFixtureCases(markdown)) {
    if (/@(?:source|config|plugin|reference)\b/.test(fixtureCase.input)) {
      skipped += fixtureCase.candidates.split(/\s+/).filter(Boolean).length;
      continue;
    }
    const candidates = fixtureCase.candidates.split(/\s+/).filter(Boolean);
    if (candidates.length === 0) candidates.push("");
    for (const candidate of candidates.filter(
      (value) => !candidateFilter || value === candidateFilter,
    )) {
      if (checked < offset) {
        checked += 1;
        continue;
      }
      if (limit > 0 && checked >= limit) break;
      checked += 1;
      const themePath = join(fixtureRoot, "parity-theme.css");
      const source = `import { style } from "stillwind/react"; const props = style(${JSON.stringify(candidate)});`;
      const tailwind = await compileTailwindCss({
        css: fixtureCase.input,
        candidates: [candidate],
        base: fixtureRoot,
        from: themePath,
      });
      const expected = minify(tailwind);
      const tailwindIsEmpty = expected === "" || expected === "@layer utilities;";
      let native: { css: string; classes: Array<{ class_name: string }>; diagnostics: unknown[] };
      try {
        native = JSON.parse(
          compileReact(source, "src/Parity.tsx", fixtureCase.input),
        ) as typeof native;
      } catch (error) {
        failures.push(`${file} ${candidate}: native exception ${String(error)}`);
        recordDifference(file, candidate, expected, "", "native-exception");
        continue;
      }
      if (native.diagnostics.length !== 0 || native.classes.length === 0) {
        if (candidate === "" && native.diagnostics.length === 0) {
          const received = minify(native.css);
          if (expected === received || (expected === "@layer utilities;" && received === "")) {
            continue;
          }
        }
        if (tailwindIsEmpty) {
          continue;
        }
        failures.push(`${file} ${candidate}: unresolved by Stillwind`);
        recordDifference(file, candidate, expected, "", "unresolved");
        if (!quiet && unresolvedShown < 20) {
          unresolvedShown += 1;
          console.error(`\n--- ${file} ${candidate} (unresolved) ---\nTailwind: ${expected}`);
          if (debugRaw)
            console.error(
              `Native diagnostics: ${JSON.stringify(native.diagnostics)}\nRaw: ${native.css}`,
            );
        }
        continue;
      }
      const actual = native.css.replaceAll(native.classes[0].class_name, selector(candidate));
      const received = minify(actual);
      if (expected === "@layer utilities;" && received === "") {
        continue;
      }
      if (expected !== received && !isJustifiedPropertyExtension(expected, received)) {
        failures.push(`${file} ${candidate}`);
        recordDifference(file, candidate, expected, received);
        if (!quiet && failures.length <= 20) {
          console.error(
            `\n--- ${file} ${candidate} ---\nTailwind: ${expected}\nStillwind: ${received}${debugRaw ? `\nRaw: ${actual}` : ""}`,
          );
        }
      }
    }
    if (limit > 0 && checked >= limit) break;
  }
  if (limit > 0 && checked >= limit) break;
}

console.log(
  `Tailwind minified parity: ${checked - failures.length}/${checked} passed (${skipped} excluded by scope)`,
);
if (analyze) {
  printCounter("Difference categories", categories);
  console.log("Category examples:");
  for (const [category, examples] of categoryExamples) {
    console.log(`  ${category}: ${examples.join(", ")}`);
  }
  for (const [category, categoryCounter] of categoryFamilies) {
    printCounter(`Candidate families for ${category}`, categoryCounter, 30);
  }
  printCounter("Declaration mismatch shapes", declarationShapes);
  printCounter("Fixture files", files);
  printCounter("Candidate families", families, 40);
  printCounter("Most frequently missing declaration properties", missingProperties, 30);
  printCounter("Most frequently extra declaration properties", extraProperties, 30);
  printCounter("Most frequently missing @property registrations", missingRegistrations, 30);
  printCounter("Most frequently extra @property registrations", extraRegistrations, 30);
}
if (failures.length > 0) {
  if (!summaryOnly) {
    console.error(failures.map((failure) => `- ${failure}`).join("\n"));
  }
  process.exitCode = 1;
}
