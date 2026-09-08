import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const packageRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

void test("the aggregate CSS entrypoint includes the built-in registry", () => {
  const css = readFileSync(path.join(packageRoot, "index.css"), "utf8");
  assert.match(css, /@import "\.\/css\/theme\.css" layer\(theme\);/);
  assert.match(css, /@import "\.\/css\/preflight\.css" layer\(base\);/);
  assert.match(css, /@import "\.\/css\/utilities\.css" layer\(utilities\);/);
  assert.match(css, /@import "\.\/css\/builtins\.css";/);
});

void test("builtins.css imports every split registry file", () => {
  const cssRoot = path.join(packageRoot, "css");
  const css = readFileSync(path.join(cssRoot, "builtins.css"), "utf8");
  const imports = [...css.matchAll(/@import "\.\/(.+\.css)";/g)].map((match) => match[1]);

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
    "interaction-variants.css",
    "selector-variants.css",
    "properties.css",
  ]);
  assert(imports.every((file) => existsSync(path.join(cssRoot, file))));
});
