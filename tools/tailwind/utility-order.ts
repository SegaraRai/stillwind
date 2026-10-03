import { readFileSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { resolve } from "node:path";
import { parseArgs } from "node:util";

import { __unstable__loadDesignSystem as load } from "tailwindcss";
import ts from "typescript";

const require = createRequire(import.meta.url);
const root = resolve(import.meta.dirname, "../..");
const propertyPath = resolve(
  root,
  "crates/design-system/src/evaluation/utilities/tailwind_property_order.rs",
);
const version = JSON.parse(
  readFileSync(require.resolve("tailwindcss/package.json"), "utf8"),
).version;
const {
  values: { check },
} = parseArgs({
  options: { check: { type: "boolean", default: false } },
});
const system = await load(readFileSync(require.resolve("tailwindcss/theme.css"), "utf8"));

// The published package bundles this internal table without exporting it.
const entry = require.resolve("tailwindcss");
const source = ts.createSourceFile(entry, readFileSync(entry, "utf8"), ts.ScriptTarget.Latest);
const tables: string[][] = [];
function visit(node: ts.Node): void {
  if (ts.isArrayLiteralExpression(node) && node.elements.every(ts.isStringLiteral)) {
    const values = node.elements.filter(ts.isStringLiteral).map((element) => element.text);
    if (values.includes("--tw-container-component") && values.includes("container-type")) {
      tables.push(values);
    }
  }
  ts.forEachChild(node, visit);
}
visit(source);
if (tables.length !== 1) {
  throw new Error("Cannot uniquely extract Tailwind's property-order table");
}
const properties = tables[0];
for (const [index, property] of properties.entries()) {
  const candidate = system.parseCandidate(`[${property}:initial]`)[0];
  const order = system.compileAstNodes(candidate)[0]?.propertySort.order;
  if (order?.length !== 1 || order[0] !== index) {
    throw new Error(`Incorrect extracted property index: ${property}`);
  }
}
const rust = `//! Generated from tailwindcss ${version} by tools/tailwind/utility-order.ts.
//! Property indices used by the Tailwind utility comparator.

pub(super) const PROPERTIES: &[&str] = &[
${properties.map((property) => `    ${JSON.stringify(property)},`).join("\n")}
];
`;
if (check) {
  if (readFileSync(propertyPath, "utf8").replace(/\r\n/g, "\n") !== rust) {
    throw new Error("Generated Rust property order is stale");
  }
} else {
  writeFileSync(propertyPath, rust);
}
console.log(
  `${check ? "Verified" : "Generated"} ${properties.length} Tailwind ${version} property indices`,
);
