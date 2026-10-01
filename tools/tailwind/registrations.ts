import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { join, resolve } from "node:path";

const require = createRequire(import.meta.url);

const root = resolve(import.meta.dirname, "../..");
const fixtureRoot = join(root, "fixtures", "tailwind-v4");
const installedTailwind = JSON.parse(
  readFileSync(require.resolve("tailwindcss/package.json"), "utf8"),
);
const tailwindLabel = `Tailwind CSS v${installedTailwind.version}`;
const sourceIndex = process.argv.indexOf("--upstream");
const upstreamArgument =
  sourceIndex === -1 ? process.env.STILLWIND_TAILWIND_UPSTREAM : process.argv[sourceIndex + 1];
const upstream = upstreamArgument ? resolve(upstreamArgument) : undefined;
const write = process.argv.includes("--write");

if (!upstream) {
  throw new Error("pass --upstream <tailwindcss-checkout> or set STILLWIND_TAILWIND_UPSTREAM");
}

function registrations(path: string, expression: RegExp): string[] {
  const source = readFileSync(path, "utf8");
  return [...source.matchAll(expression)]
    .map((match) => match[1])
    .filter((name, index, names) => names.indexOf(name) === index)
    .sort((a, b) => a.localeCompare(b, "en"));
}

function lines(values: readonly string[], width = 10): string {
  return values.reduce((output, value, index) => {
    output += `${value}${index % width === width - 1 || index === values.length - 1 ? "\n" : " "}`;
    return output;
  }, "");
}

const utilitiesPath = join(upstream, "packages", "tailwindcss", "src", "utilities.ts");
const variantsPath = join(upstream, "packages", "tailwindcss", "src", "variants.ts");
const intellisenseSnapshotPath = join(
  upstream,
  "packages",
  "tailwindcss",
  "src",
  "__snapshots__",
  "intellisense.test.ts.snap",
);
const staticUtilities = registrations(utilitiesPath, /staticUtility\('([^']+)'/g);
const functionalUtilities = registrations(utilitiesPath, /functionalUtility\('([^']+)'/g);
const staticVariants = [
  ...new Set([
    ...registrations(variantsPath, /staticVariant\('([^']+)'/g),
    ...registrations(variantsPath, /variants\.static\(\s*'([^']+)'/g),
  ]),
].sort((a, b) => a.localeCompare(b, "en"));
const functionalVariants = registrations(variantsPath, /variants\.functional\(\s*'([^']+)'/g);
const compoundVariants = registrations(variantsPath, /variants\.compound\('([^']+)'/g);
const intellisenseSnapshot = readFileSync(intellisenseSnapshotPath, "utf8");
const classListSnapshot = /exports\[`getClassList 1`\] = `\n([\s\S]*?)\n`;/m.exec(
  intellisenseSnapshot,
)?.[1];
if (!classListSnapshot) {
  throw new Error("could not find the default getClassList snapshot");
}
const defaultCompletions = [...classListSnapshot.matchAll(/^  "([^"]+)",?$/gm)].map(
  (match) => match[1],
);
const variantSnapshot = /exports\[`getVariants 1`\] = `\n([\s\S]*?)\n`;/m.exec(
  intellisenseSnapshot,
)?.[1];
if (!variantSnapshot) {
  throw new Error("could not find the default getVariants snapshot");
}

const catalog = `# ${tailwindLabel} direct registrations\n\nThis file is generated from the upstream source matching the tools dependency. It records every\ndirect \`staticUtility\`, \`functionalUtility\`, and static/functional/compound\nvariant registration. Template-generated registrations and parameterized grammar\nare covered by the dedicated candidate and dynamic-variant fixtures.\n\n## Static utilities (${staticUtilities.length})\n\n\`\`\`text\n${lines(staticUtilities)}\`\`\`\n\n## Functional utility roots (${functionalUtilities.length})\n\n\`\`\`text\n${lines(functionalUtilities)}\`\`\`\n\n## Static variants (${staticVariants.length})\n\n\`\`\`text\n${lines(staticVariants)}\`\`\`\n\n## Functional variant roots (${functionalVariants.length})\n\n\`\`\`text\n${lines(functionalVariants)}\`\`\`\n\n## Compound variant roots (${compoundVariants.length})\n\n\`\`\`text\n${lines(compoundVariants)}\`\`\`\n`;
const completionCatalog = `# ${tailwindLabel} default completion catalog\n\nThis is the complete default-theme class list from upstream's\n\`getClassList\` IntelliSense snapshot. It includes utility values generated\nfrom theme namespaces and loop-based registrations, complementing the direct\nregistration catalog.\n\n## Default completions (${defaultCompletions.length})\n\n\`\`\`text\n${lines(defaultCompletions, 8)}\`\`\`\n`;
const variantCatalog = `# ${tailwindLabel} variant catalog\n\nThis is the complete default \`getVariants\` IntelliSense snapshot from\nupstream. It preserves every variant name, its parameter suggestions, and\nmetadata used by the compiler's public completion surface. The executable\nfixtures cover every static registration and each dynamic grammar root.\n\n\`\`\`text\n${variantSnapshot.trim()}\n\`\`\`\n`;

const input =
  '@import "tailwindcss/theme" layer(theme);\n@import "tailwindcss/utilities" layer(utilities);';
const candidateBlocks = new Map([
  ["builtins.static-utilities", lines(staticUtilities)],
  [
    "builtins.functional-utility-roots-with-arbitrary-values",
    lines(functionalUtilities.map((name) => `${name}-[initial]`)),
  ],
  ["builtins.static-variants", lines(staticVariants.map((name) => `${name}:bg-red-500`))],
]);
const fixtureTemplate = `# Exhaustive direct built-in registrations\n\n${[...candidateBlocks]
  .map(
    ([name, candidates]) =>
      `## \`${name}\`\n\n### Input CSS\n\n\`\`\`css\n${input}\n\`\`\`\n\n### Candidates\n\n\`\`\`text\n${candidates}\`\`\`\n\n### Expected CSS\n\n\`\`\`css\n\n\`\`\``,
  )
  .join("\n\n")}\n`;
const completionFixtureTemplate = `# Exhaustive default-theme built-in completions\n\n## \`builtins.default-theme-completions\`\n\n### Input CSS\n\n\`\`\`css\n${input}\n\`\`\`\n\n### Candidates\n\n\`\`\`text\n${lines(defaultCompletions, 8)}\`\`\`\n\n### Expected CSS\n\n\`\`\`css\n\n\`\`\`\n`;

function synchronizeCandidates(markdown: string): string {
  let next = markdown;
  for (const [name, candidates] of candidateBlocks) {
    const pattern = new RegExp(
      `(## \`${name}\`[\\s\\S]*?### Candidates\\n\\n\`\`\`text\\n)([\\s\\S]*?)(\\n\`\`\`)`,
    );
    if (!pattern.test(next)) {
      throw new Error(`builtins fixture is missing ${name}`);
    }
    next = next.replace(pattern, `$1${candidates.trimEnd()}$3`);
  }
  return next;
}

function synchronizeCompletionCandidates(markdown: string): string {
  const pattern =
    /(## `builtins\.default-theme-completions`[\s\S]*?### Candidates\n\n```text\n)([\s\S]*?)(\n```)/;
  if (!pattern.test(markdown)) {
    throw new Error("default-completions fixture is missing its case");
  }
  return markdown.replace(pattern, `$1${lines(defaultCompletions, 8).trimEnd()}$3`);
}

const outputs = [
  [join(fixtureRoot, "catalog", "direct-registrations.md"), catalog],
  [join(fixtureRoot, "catalog", "default-completions.md"), completionCatalog],
  [join(fixtureRoot, "catalog", "variants.md"), variantCatalog],
];

let changed = false;
for (const [path, output] of outputs) {
  let current = "";
  try {
    current = readFileSync(path, "utf8").replaceAll("\r\n", "\n");
  } catch {
    // The first generation creates the catalog and fixture template.
  }
  if (current !== output) {
    changed = true;
    if (write) {
      mkdirSync(resolve(path, ".."), { recursive: true });
      writeFileSync(path, output);
    } else {
      console.error(`${path} is stale; run with --write`);
    }
  }
}

const fixturePath = join(fixtureRoot, "cases", "builtins.fixture.md");
let currentFixture = "";
try {
  currentFixture = readFileSync(fixturePath, "utf8").replaceAll("\r\n", "\n");
} catch {
  // The first generation creates the fixture template.
}
const nextFixture = currentFixture ? synchronizeCandidates(currentFixture) : fixtureTemplate;
if (currentFixture !== nextFixture) {
  changed = true;
  if (write) {
    writeFileSync(fixturePath, nextFixture);
  } else {
    console.error(`${fixturePath} is stale; run with --write`);
  }
}

const completionFixturePath = join(fixtureRoot, "cases", "default-completions.fixture.md");
let currentCompletionFixture = "";
try {
  currentCompletionFixture = readFileSync(completionFixturePath, "utf8").replaceAll("\r\n", "\n");
} catch {
  // The first generation creates the fixture template.
}
const nextCompletionFixture = currentCompletionFixture
  ? synchronizeCompletionCandidates(currentCompletionFixture)
  : completionFixtureTemplate;
if (currentCompletionFixture !== nextCompletionFixture) {
  changed = true;
  if (write) {
    writeFileSync(completionFixturePath, nextCompletionFixture);
  } else {
    console.error(`${completionFixturePath} is stale; run with --write`);
  }
}

if (changed && !write) {
  process.exitCode = 1;
} else {
  console.log(
    `${write ? "Generated" : "Verified"} ${staticUtilities.length} static utilities, ${functionalUtilities.length} functional utility roots, ${staticVariants.length} static variants, ${functionalVariants.length} functional variant roots, ${compoundVariants.length} compound variant roots, and ${defaultCompletions.length} default completions.`,
  );
}
