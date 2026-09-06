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
    "core-utilities.css",
    "discrete-utilities.css",
    "functional-utilities.css",
    "interaction-variants.css",
    "static-utilities.css",
    "arbitrary-utilities.css",
    "mask-utilities.css",
    "effect-utilities.css",
    "selector-variants.css",
    "properties.css",
  ]);
  assert(imports.every((file) => existsSync(path.join(cssRoot, file))));
});
