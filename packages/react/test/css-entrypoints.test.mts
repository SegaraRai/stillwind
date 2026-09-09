import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const packageRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const cssRoot = path.resolve(packageRoot, "..", "..", "css");

void test("the aggregate CSS entrypoint includes the built-in registry", () => {
  const css = readFileSync(path.join(packageRoot, "index.css"), "utf8");
  assert.match(css, /@import "\.\/theme\.css" layer\(theme\);/);
  assert.match(css, /@import "\.\/preflight\.css" layer\(base\);/);
  assert.match(css, /@import "\.\/utilities\.css" layer\(utilities\);/);
  assert.match(css, /@import "\.\/builtins\.css";/);
});

void test("package CSS subpaths forward to the workspace CSS sources", () => {
  for (const file of ["theme.css", "preflight.css", "utilities.css", "builtins.css"]) {
    const css = readFileSync(path.join(packageRoot, file), "utf8");
    assert.equal(css, `@import "../../css/${file}";\n`);
  }
});

void test("builtins.css imports every split registry file", () => {
  const css = readFileSync(path.join(cssRoot, "builtins.css"), "utf8");
  const imports = [...css.matchAll(/@import "\.\/builtins\/(.+\.css)";/g)].map((match) => match[1]);

  assert.deepEqual(imports, [
    "registry-metadata.css",
    "relationship-metadata.css",
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
    "relationship-variants.css",
    "structural-variants.css",
    "state-variants.css",
    "dynamic-selector-variants.css",
    "conditional-variants.css",
    "properties.css",
  ]);
  assert(imports.every((file) => existsSync(path.join(cssRoot, "builtins", file))));
});
