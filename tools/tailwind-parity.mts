import { readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";

import { compileReact } from "@stillwind/native";
import { transform } from "lightningcss";

import { parseMarkdownFixtureCases } from "./lib/markdown-fixtures.ts";
import { compileTailwindCss } from "./lib/tailwind-compiler.ts";

const root = resolve(import.meta.dirname, "..");
const fixtureRoot = join(root, "fixtures", "tailwind-v4");
const caseRoot = join(fixtureRoot, "cases");
const limit = Number(process.env.STILLWIND_PARITY_LIMIT ?? "0");
const fileFilter = process.env.STILLWIND_PARITY_FILE;
const candidateFilter = process.env.STILLWIND_PARITY_CANDIDATE;

function minify(css: string): string {
  return Buffer.from(
    transform({
      filename: "parity.css",
      code: Buffer.from(css.replaceAll(/\/\*[\s\S]*?\*\//g, "")),
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
    for (const candidate of fixtureCase.candidates
      .split(/\s+/)
      .filter((value) => !candidateFilter || value === candidateFilter)) {
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
        if (unresolvedShown < 20) {
          unresolvedShown += 1;
          console.error(`\n--- ${file} ${candidate} (unresolved) ---\nTailwind: ${expected}`);
        }
        continue;
      }
      const actual = native.css.replaceAll(native.classes[0].class_name, selector(candidate));
      const received = minify(actual);
      if (expected === "@layer utilities;" && received === "") {
        continue;
      }
      if (expected !== received) {
        failures.push(`${file} ${candidate}`);
        if (failures.length <= 20) {
          console.error(
            `\n--- ${file} ${candidate} ---\nTailwind: ${expected}\nStillwind: ${received}`,
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
if (failures.length > 0) {
  console.error(failures.map((failure) => `- ${failure}`).join("\n"));
  process.exitCode = 1;
}
