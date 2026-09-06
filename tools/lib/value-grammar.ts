import { readFileSync } from "node:fs";

export interface ValueGrammarCase {
  candidate: string;
  accepted: boolean;
}

export function readValueGrammarCases(path: string): ValueGrammarCase[] {
  const catalog = readFileSync(path, "utf8");
  return [
    ...readSection(catalog, "Padding: accepted without brackets", true),
    ...readSection(catalog, "Padding: rejected without brackets", false),
    ...readSection(catalog, "Border width: accepted without brackets", true),
    ...readSection(catalog, "Border width: rejected without brackets", false),
    ...readSection(catalog, "Z index: accepted without brackets", true),
    ...readSection(catalog, "Z index: rejected without brackets", false),
  ];
}

function readSection(catalog: string, heading: string, accepted: boolean): ValueGrammarCase[] {
  const marker = `## ${heading}\n\n\`\`\`text\n`;
  const start = catalog.indexOf(marker);
  if (start < 0) {
    throw new Error(`Missing value grammar section: ${heading}`);
  }
  const contentStart = start + marker.length;
  const end = catalog.indexOf("\n```", contentStart);
  if (end < 0) {
    throw new Error(`Unclosed value grammar section: ${heading}`);
  }
  return catalog
    .slice(contentStart, end)
    .trim()
    .split(/\s+/)
    .filter(Boolean)
    .map((candidate) => ({ candidate, accepted }));
}
