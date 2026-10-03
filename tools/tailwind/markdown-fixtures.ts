import { readFileSync, readdirSync, writeFileSync } from "node:fs";
import { basename, join } from "node:path";

export interface MarkdownFixtureCase {
  name: string;
  input: string;
  candidates: string;
  expectedStart: number;
  expectedEnd: number;
}

interface VerifyMarkdownFixturesOptions {
  directory: string;
  write: boolean;
  compileCase: (fixtureCase: MarkdownFixtureCase) => Promise<string> | string;
  maxFiles?: number;
}

export function parseMarkdownFixtureCases(markdown: string): MarkdownFixtureCase[] {
  const pattern =
    /## `([^`]+)`\r?\n\n### Input CSS\r?\n\r?\n```css\r?\n([\s\S]*?)\r?\n```\r?\n\r?\n### Candidates\r?\n\r?\n```text\r?\n([\s\S]*?)\r?\n```\r?\n\r?\n### Expected CSS\r?\n\r?\n```css\r?\n([\s\S]*?)\r?\n```/g;
  return [...markdown.matchAll(pattern)].map((match) => ({
    name: match[1],
    input: match[2],
    candidates: match[3].trim().replace(/\s+/g, " "),
    expectedStart:
      match.index +
      match[0].lastIndexOf("### Expected CSS\n\n```css\n") +
      "### Expected CSS\n\n```css\n".length,
    expectedEnd: match.index + match[0].length - "\n```".length,
  }));
}

export async function verifyMarkdownFixtures({
  directory,
  write,
  compileCase,
  maxFiles = 500,
}: VerifyMarkdownFixturesOptions) {
  const files = readdirSync(directory)
    .filter((file) => file.endsWith(".fixture.md"))
    .sort();
  if (files.length > maxFiles) {
    throw new Error(`fixture file budget exceeded (maximum: ${maxFiles})`);
  }

  let changed = false;
  let caseCount = 0;
  for (const file of files) {
    const path = join(directory, file);
    const markdown = readFileSync(path, "utf8").replaceAll("\r\n", "\n");
    const cases = parseMarkdownFixtureCases(markdown);
    if (cases.length === 0) {
      throw new Error(`${file} does not contain a valid Markdown fixture case`);
    }
    caseCount += cases.length;

    const replacements = [];
    for (const fixtureCase of cases) {
      replacements.push({ fixtureCase, css: await compileCase(fixtureCase) });
    }

    let next = markdown;
    for (const { fixtureCase, css } of replacements.reverse()) {
      next = `${next.slice(0, fixtureCase.expectedStart)}${css}${next.slice(fixtureCase.expectedEnd)}`;
    }
    if (next !== markdown) {
      changed = true;
      if (write) {
        writeFileSync(path, next);
      } else {
        console.error(`${basename(path)} is stale; run vp run generate:fixtures:tailwind`);
      }
    }
  }
  return { changed, caseCount, fileCount: files.length };
}
