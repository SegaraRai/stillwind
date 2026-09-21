import assert from "node:assert/strict";
import { existsSync, readFileSync, readdirSync } from "node:fs";
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
    exports: Record<string, string | Record<string, string>>;
  };
  const pending = Object.values(manifest.exports)
    .filter((target): target is string => typeof target === "string" && target.endsWith(".css"))
    .map((target) => path.resolve(packageRoot, target));
  const visited = new Set<string>();
  while (pending.length) {
    const file = pending.pop()!;
    if (visited.has(file)) {
      continue;
    }
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
  const imports = [
    ...css.matchAll(/@import "\.\/(builtin-(?:utilities|variants|metadata)\/.+\.css)";/g),
  ].map((match) => match[1]);

  assert.deepEqual(imports, [
    "builtin-metadata/tw-registry-metadata.css",
    "builtin-metadata/tw-relationship-metadata.css",
    "builtin-utilities/tw-layout.css",
    "builtin-utilities/tw-spacing-sizing.css",
    "builtin-utilities/tw-flex-grid.css",
    "builtin-utilities/tw-typography.css",
    "builtin-utilities/tw-backgrounds.css",
    "builtin-utilities/tw-borders.css",
    "builtin-utilities/tw-effects.css",
    "builtin-utilities/tw-masks.css",
    "builtin-utilities/tw-transforms.css",
    "builtin-utilities/tw-motion.css",
    "builtin-utilities/tw-interactivity.css",
    "builtin-utilities/tw-svg.css",
    "builtin-variants/tw-relationship.css",
    "builtin-variants/tw-structural.css",
    "builtin-variants/tw-state.css",
    "builtin-variants/tw-dynamic-selector.css",
    "builtin-variants/tw-conditional.css",
    "builtin-variants/sw-conditional.css",
    "builtin-metadata/tw-properties.css",
  ]);
  const files = ["builtin-utilities", "builtin-variants", "builtin-metadata"].flatMap((directory) =>
    readdirSync(path.join(cssRoot, directory)).map((file) => `${directory}/${file}`),
  );
  assert.deepEqual([...imports].sort(), files.sort());
  assert(files.every((file) => /\/(tw|sw)-.+\.css$/.test(file)));
  assert(files.every((file) => !/-(utilities|variants)\.css$/.test(file)));
  assert(imports.every((file) => existsSync(path.join(cssRoot, file))));
});
