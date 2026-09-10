import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const packageRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const cssRoot = path.join(packageRoot, "css");

void test("the aggregate CSS entrypoint includes the built-in registry", () => {
  const css = readFileSync(path.join(packageRoot, "index.css"), "utf8");
  assert.match(css, /@import "\.\/css\/theme\.css" layer\(theme\);/);
  assert.match(css, /@import "\.\/css\/preflight\.css" layer\(base\);/);
  assert.match(css, /@import "\.\/css\/utilities\.css" layer\(utilities\);/);
  assert.match(css, /@import "\.\/css\/builtins\.css";/);
});

void test("every exported CSS import stays inside the published package", () => {
  const manifest = JSON.parse(readFileSync(path.join(packageRoot, "package.json"), "utf8")) as {
    exports: Record<string, string>;
  };
  const pending = Object.values(manifest.exports)
    .filter((target) => target.endsWith(".css"))
    .map((target) => path.resolve(packageRoot, target));
  const visited = new Set<string>();
  while (pending.length) {
    const file = pending.pop()!;
    if (visited.has(file)) continue;
    visited.add(file);
    assert.ok(file.startsWith(packageRoot + path.sep), `CSS import escaped the package: ${file}`);
    const css = readFileSync(file, "utf8");
    for (const match of css.matchAll(/@import\s+["']([^"']+)["']/g)) {
      assert.ok(match[1].startsWith("."));
      pending.push(path.resolve(path.dirname(file), match[1]));
    }
  }
  assert.ok(visited.size > 4);
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
