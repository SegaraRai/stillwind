import { readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

import { compileTailwindCss } from "./lib/tailwind-compiler.ts";

const root = resolve(import.meta.dirname, "..");
const fixtureRoot = join(root, "fixtures", "tailwind-v4");
const catalogPath = join(fixtureRoot, "catalog", "direct-registrations.md");
const registryPath = join(root, "crates", "shorth-tailwind", "assets", "builtins.css");
const write = process.argv.includes("--write");

const catalog = readFileSync(catalogPath, "utf8");
const staticUtilities = section(catalog, "Static utilities").trim().split(/\s+/u);
const input = '@import "tailwindcss/utilities" layer(utilities);';
const rules = [];
const skipped = [];

for (const candidate of staticUtilities) {
  const css = await compileTailwindCss({
    css: input,
    candidates: [candidate],
    base: fixtureRoot,
  });
  const body = firstUtilityBody(css);
  if (!body || body.includes("@")) {
    skipped.push(candidate);
    continue;
  }
  rules.push(`@utility ${candidate} {\n${indent(body.trim(), 2)}\n}`);
}

if (skipped.length > 0) {
  throw new Error(
    `cannot lower ${skipped.length} static utilities without nested-rule support: ${skipped.join(" ")}`,
  );
}

const generated = `/* GENERATED STATIC UTILITIES: run vp run registry:static --write. */\n${rules.join("\n")}`;
const current = readFileSync(registryPath, "utf8").replaceAll("\r\n", "\n");
const start = "/* GENERATED STATIC UTILITIES:";
const end = "/* END GENERATED STATIC UTILITIES */";
const startIndex = current.indexOf(start);
const endIndex = current.indexOf(end);
const replacement = `${generated}\n${end}`;
const next =
  startIndex === -1 || endIndex === -1
    ? `${current.trimEnd()}\n\n${replacement}\n`
    : `${current.slice(0, startIndex)}${replacement}${current.slice(endIndex + end.length)}`;

if (next !== current) {
  if (write) writeFileSync(registryPath, next);
  else console.error(`${registryPath} is stale; run with --write`);
}

console.log(`Lowered ${rules.length} Tailwind static utilities into the CSS registry.`);
if (!write && next !== current) process.exitCode = 1;

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
