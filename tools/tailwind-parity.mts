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
  return candidate.replaceAll(/[^a-zA-Z0-9_-]/g, (value) => `\\${value}`);
}

const failures: string[] = [];
let checked = 0;
let skipped = 0;
for (const file of readdirSync(caseRoot)
  .filter((name) => name.endsWith(".fixture.md"))
  .sort()) {
  const markdown = readFileSync(join(caseRoot, file), "utf8");
  for (const fixtureCase of parseMarkdownFixtureCases(markdown)) {
    if (/@(?:source|config|plugin|reference)\b/.test(fixtureCase.input)) {
      skipped += fixtureCase.candidates.split(/\s+/).filter(Boolean).length;
      continue;
    }
    for (const candidate of fixtureCase.candidates.split(/\s+/).filter(Boolean)) {
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
        failures.push(`${file} ${candidate}: unresolved by Stillwind`);
        continue;
      }
      const actual = native.css.replaceAll(native.classes[0].class_name, selector(candidate));
      const expected = minify(tailwind);
      const received = minify(actual);
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
