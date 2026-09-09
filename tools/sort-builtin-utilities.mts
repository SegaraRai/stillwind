import { readFileSync, writeFileSync } from "node:fs";
import { basename, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(import.meta.dirname, "..");
const builtins = join(root, "css", "builtins");
const write = process.argv.includes("--write");
const utilityFiles = [
  "layout-utilities.css",
  "spacing-sizing-utilities.css",
  "flex-grid-utilities.css",
  "typography-utilities.css",
  "backgrounds-utilities.css",
  "borders-utilities.css",
  "effects-utilities.css",
  "masks-utilities.css",
  "transforms-utilities.css",
  "motion-utilities.css",
  "interactivity-utilities.css",
  "svg-utilities.css",
];

function tailwindPropertyOrder(): string[] {
  const entry = fileURLToPath(import.meta.resolve("tailwindcss"));
  const source = readFileSync(entry, "utf8");
  const start = source.indexOf('["container-type","pointer-events","visibility","position"');
  if (start === -1) throw new Error(`could not find Tailwind property order in ${entry}`);
  const marker = '"forced-color-adjust"]';
  const end = source.indexOf(marker, start);
  if (end === -1) throw new Error(`could not find the end of Tailwind property order in ${entry}`);
  return JSON.parse(source.slice(start, end + marker.length)) as string[];
}

type UtilityBlock = {
  prefix: string;
  source: string;
  name: string;
  declarations: string[];
  properties: number[];
  declarationCount: number;
  originalIndex: number;
};

function closingBrace(source: string, open: number): number {
  let depth = 0;
  let quote: string | undefined;
  let escaped = false;
  for (let index = open; index < source.length; index++) {
    const character = source[index];
    if (quote) {
      if (escaped) escaped = false;
      else if (character === "\\") escaped = true;
      else if (character === quote) quote = undefined;
      continue;
    }
    if (character === '"' || character === "'") quote = character;
    else if (character === "{") depth++;
    else if (character === "}" && --depth === 0) return index;
  }
  throw new Error(`unterminated @utility block at offset ${open}`);
}

function compareArrays(left: readonly number[], right: readonly number[]): number {
  const length = Math.max(left.length, right.length);
  for (let index = 0; index < length; index++) {
    const difference =
      (left[index] ?? Number.POSITIVE_INFINITY) - (right[index] ?? Number.POSITIVE_INFINITY);
    if (difference !== 0) return difference;
  }
  return 0;
}

function compareNames(left: string, right: string): number {
  return left < right ? -1 : left > right ? 1 : 0;
}

function parseFile(path: string, propertyIndexes: ReadonlyMap<string, number>) {
  const source = readFileSync(path, "utf8");
  const starts = [...source.matchAll(/^@utility\s+([^\s{]+)[^{]*\{/gm)];
  if (starts.length === 0) return { source, preamble: source, blocks: [], suffix: "" };

  const blocks: UtilityBlock[] = [];
  let previousEnd = 0;
  for (const [originalIndex, match] of starts.entries()) {
    const start = match.index;
    const open = start + match[0].lastIndexOf("{");
    const end = closingBrace(source, open) + 1;
    const body = source.slice(open + 1, end - 1);
    const declarations = [...body.matchAll(/^\s*([\w-]+)\s*:/gm)].map((item) => item[1]);
    const twSort = [...body.matchAll(/^\s*--tw-sort\s*:\s*([\w-]+)/gm)].at(-1)?.[1];
    const properties = [
      ...new Set(
        (twSort ? [twSort] : declarations)
          .map((property) => propertyIndexes.get(property))
          .filter((index): index is number => index !== undefined),
      ),
    ].sort((left, right) => left - right);
    blocks.push({
      prefix: source.slice(previousEnd, start),
      source: source.slice(start, end),
      name: match[1],
      declarations,
      properties,
      declarationCount: declarations.length,
      originalIndex,
    });
    previousEnd = end;
  }
  return {
    source,
    preamble: blocks[0].prefix,
    blocks: blocks.map((block, index) => (index === 0 ? { ...block, prefix: "" } : block)),
    suffix: source.slice(previousEnd),
  };
}

function familyName(
  block: UtilityBlock,
  wildcardRoots: ReadonlyMap<string, ReadonlySet<string>>,
): string {
  const positive = block.name.replace(/^-/, "");
  const root = [...wildcardRoots]
    .filter(
      ([candidate, properties]) =>
        (positive === candidate ||
          positive === `${candidate}-*` ||
          positive.startsWith(`${candidate}-`)) &&
        block.declarations.some((property) => properties.has(property)),
    )
    .map(([candidate]) => candidate)
    .sort((left, right) => right.length - left.length)[0];
  return root ? `wildcard:${root}` : `exact:${positive}`;
}

function variantRank(name: string): number {
  const negative = name.startsWith("-");
  const wildcard = name.endsWith("*");
  if (!negative && !wildcard) return 0;
  if (!negative) return 1;
  if (!wildcard) return 2;
  return 3;
}

function sortFile(path: string, propertyIndexes: ReadonlyMap<string, number>): string {
  const parsed = parseFile(path, propertyIndexes);
  if (parsed.blocks.length === 0) return parsed.source;
  const wildcardRoots = new Map<string, Set<string>>();
  for (const block of parsed.blocks) {
    const name = block.name.replace(/^-/, "");
    if (!name.endsWith("-*")) continue;
    const root = name.slice(0, -2);
    const properties = wildcardRoots.get(root) ?? new Set<string>();
    for (const property of block.declarations) properties.add(property);
    wildcardRoots.set(root, properties);
  }
  const families = new Map<string, UtilityBlock[]>();
  for (const block of parsed.blocks) {
    const family = familyName(block, wildcardRoots);
    const members = families.get(family) ?? [];
    members.push(block);
    families.set(family, members);
  }

  const sortedFamilies = [...families.entries()].sort(([leftName, left], [rightName, right]) => {
    const leftProperties = [...new Set(left.flatMap((block) => block.properties))].sort(
      (a, b) => a - b,
    );
    const rightProperties = [...new Set(right.flatMap((block) => block.properties))].sort(
      (a, b) => a - b,
    );
    return (
      compareArrays(leftProperties, rightProperties) ||
      Math.max(...right.map((block) => block.declarationCount)) -
        Math.max(...left.map((block) => block.declarationCount)) ||
      compareNames(
        leftName.slice(leftName.indexOf(":") + 1),
        rightName.slice(rightName.indexOf(":") + 1),
      )
    );
  });

  const blocks = sortedFamilies.flatMap(([, members]) =>
    members.sort(
      (left, right) =>
        variantRank(left.name) - variantRank(right.name) ||
        compareNames(left.name, right.name) ||
        left.originalIndex - right.originalIndex,
    ),
  );
  return `${parsed.preamble}${blocks
    .map((block) => `${block.prefix}${block.source}`)
    .join("")}${parsed.suffix}`;
}

const propertyIndexes = new Map(
  tailwindPropertyOrder().map((property, index) => [property, index] as const),
);
const changed: string[] = [];
for (const file of utilityFiles) {
  const path = join(builtins, file);
  const source = readFileSync(path, "utf8");
  const sorted = sortFile(path, propertyIndexes);
  if (source === sorted) continue;
  changed.push(basename(path));
  if (write) writeFileSync(path, sorted);
}

if (changed.length > 0 && !write) {
  throw new Error(`built-in utility order is stale: ${changed.join(", ")}`);
}
console.log(
  changed.length === 0
    ? `Verified ${utilityFiles.length} built-in utility files.`
    : `Sorted ${changed.length} built-in utility files.`,
);
