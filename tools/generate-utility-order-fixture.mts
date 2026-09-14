import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

import { __unstable__loadDesignSystem as load } from "tailwindcss";

const root = resolve(import.meta.dirname, "..");
const propertyPath = resolve(
  root,
  "crates/design-system/src/evaluation/utilities/tailwind-property-order.txt",
);
const upstreamIndex = process.argv.indexOf("--upstream");
if (upstreamIndex !== -1) {
  const source = readFileSync(
    resolve(process.argv[upstreamIndex + 1], "packages/tailwindcss/src/property-order.ts"),
    "utf8",
  );
  const properties =
    [...source.replace(/\/\/[^\n]*/g, "").matchAll(/'([^']+)'/g)]
      .map((match) => match[1])
      .join("\n") + "\n";
  if (process.argv.includes("--check")) {
    if (readFileSync(propertyPath, "utf8") !== properties)
      throw new Error("Property ordering data is stale");
  } else writeFileSync(propertyPath, properties);
}
const version = JSON.parse(
  readFileSync(resolve(import.meta.dirname, "node_modules/tailwindcss/package.json"), "utf8"),
).version;
if (version !== "4.3.3") throw new Error(`Update the ordering contract for Tailwind ${version}`);
const css = `
@utility order-many { color:red; color:blue; --extra:; }
@utility order-one { color:red; }
@utility order-second { color:red; background-color:blue; }
@utility order-unknown { unknown:1; }
@utility order-empty { --empty:; }
@utility order-sort { --tw-sort:margin; color:red; display:block; }
@utility order-invalid-sort { --tw-sort:invalid; color:red; }
@utility order-sort-important { --tw-sort:margin!important; color:red; }
@utility order-nested { color:red; @media (width > 1px) { display:block; --extra:; } }
@utility order-2 { color:red; }
@utility order-02 { color:red; }
@utility order-10 { color:red; }
`;
const system = await load(
  readFileSync(resolve(import.meta.dirname, "node_modules/tailwindcss/theme.css"), "utf8") + css,
);
for (const [index, property] of readFileSync(propertyPath, "utf8")
  .trimEnd()
  .split(/\r?\n/)
  .entries()) {
  const candidate = system.parseCandidate(`[${property}:initial]`)[0];
  const order = system.compileAstNodes(candidate)[0]?.propertySort.order;
  if (order?.length !== 1 || order[0] !== index)
    throw new Error(`Incorrect property index: ${property}`);
}
const candidates = [
  ...system.getClassList().map(([name]) => name),
  "[unknown:1]",
  "[color:red]",
  "[--x:1]",
  "[margin:1px]",
  "order-2!",
  "order-10!",
  "w-2.5",
  "w-10",
];
const signatures = new Set<string>();
const keys = [];
for (const candidate of candidates) {
  const asts = system.parseCandidate(candidate).flatMap((value) => system.compileAstNodes(value));
  if (asts.length !== 1) continue;
  const { propertySort } = asts[0];
  const signature = JSON.stringify([
    candidate.replace(
      /(?:\d.*|red.*|blue.*|slate.*|gray.*|zinc.*|neutral.*|stone.*|amber.*|yellow.*|lime.*|green.*|emerald.*|teal.*|cyan.*|sky.*|indigo.*|violet.*|purple.*|fuchsia.*|pink.*|rose.*)$/,
      "*",
    ),
    propertySort,
  ]);
  if (signatures.has(signature) && !candidate.startsWith("order-")) continue;
  signatures.add(signature);
  keys.push({ candidate, ...propertySort });
}
const ordered = system
  .getClassOrder(keys.map((key) => key.candidate))
  .sort((a, b) => (a[1]! < b[1]! ? -1 : a[1]! > b[1]! ? 1 : 0))
  .map(([candidate]) => candidate);
const output =
  JSON.stringify(
    {
      version,
      css,
      stillwindCss: css.replace(
        /@utility (\S+) \{ --tw-sort:([^;!]+)(?:!important)?;/g,
        "@utility $1 (tw-sort: $2) {",
      ),
      keys,
      ordered,
    },
    null,
    2,
  ) + "\n";
const path = resolve(root, "fixtures/tailwind-v4/utility-order.json");
if (process.argv.includes("--check")) {
  if (JSON.stringify(JSON.parse(readFileSync(path, "utf8"))) !== JSON.stringify(JSON.parse(output)))
    throw new Error("Utility-order fixture is stale");
} else writeFileSync(path, output);
console.log(`Verified ${keys.length} Tailwind ${version} utility ordering keys`);
