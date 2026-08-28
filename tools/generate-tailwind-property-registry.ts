import { readFileSync, readdirSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const fixtureDirectory = join(root, "fixtures", "tailwind-v4", "cases");
const outputPath = join(root, "crates", "shorth-css", "assets", "tailwind-v4-properties.css");
const write = process.argv.includes("--write");
const definitions = new Map();

for (const file of readdirSync(fixtureDirectory)
  .filter((name) => name.endsWith(".fixture.md"))
  .sort()) {
  const fixture = readFileSync(join(fixtureDirectory, file), "utf8").replaceAll("\r\n", "\n");
  for (const match of fixture.matchAll(/@property\s+(--[\w-]+)\s*\{([\s\S]*?)\n\}/gu)) {
    const body = match[2]
      .trim()
      .split("\n")
      .map((line) => `  ${line.trim()}`)
      .join("\n");
    definitions.set(match[1], `@property ${match[1]} {\n${body}\n}`);
  }
}

if (definitions.size === 0)
  throw new Error(`could not find property registrations in ${fixtureDirectory}`);
const output = `/* GENERATED FROM THE TAILWIND V4 FIXTURE. */\n${[...definitions.values()].join("\n\n")}\n`;
if (write) writeFileSync(outputPath, output);
console.log(`Extracted ${definitions.size} Tailwind property registrations.`);
