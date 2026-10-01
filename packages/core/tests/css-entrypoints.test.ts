import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { expect, test } from "vite-plus/test";

const packageRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
test("every exported CSS import stays inside the published package", () => {
  const manifest = JSON.parse(readFileSync(path.join(packageRoot, "package.json"), "utf8")) as {
    exports: Record<string, string | Record<string, string>>;
  };
  const pending = Object.values(manifest.exports)
    .filter((target): target is string => typeof target === "string" && target.endsWith(".css"))
    .map((target) => path.resolve(packageRoot, target));
  expect(pending.length, "the package exports CSS entrypoints").toBeGreaterThan(0);
  const visited = new Set<string>();
  while (pending.length) {
    const file = pending.pop()!;
    if (visited.has(file)) {
      continue;
    }
    visited.add(file);
    expect(
      file.startsWith(packageRoot + path.sep),
      `CSS import escaped the package: ${file}`,
    ).toBeTruthy();
    const css = readFileSync(file, "utf8");
    for (const match of css.matchAll(/@import\s+["']([^"']+)["']/g)) {
      expect(match[1].startsWith(".")).toBeTruthy();
      pending.push(path.resolve(path.dirname(file), match[1]));
    }
  }
});
