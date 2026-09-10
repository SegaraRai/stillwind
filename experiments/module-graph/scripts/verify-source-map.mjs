import assert from "node:assert/strict";
import fs from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";

import { AnyMap, originalPositionFor } from "@jridgewell/trace-mapping";

export async function verifyMacroSourceMap(directory, sourcePath, source, macro) {
  const originalOffset = source.indexOf(macro);
  assert.ok(originalOffset >= 0);
  const prefix = source.slice(0, originalOffset);
  const expected = {
    line: prefix.split("\n").length,
    column: originalOffset - prefix.lastIndexOf("\n") - 1,
  };
  const candidates = [];
  for (const file of await fs.readdir(directory, { recursive: true })) {
    if (!/\.[cm]?js$/.test(file)) continue;
    const target = path.join(directory, file);
    const code = await fs.readFile(target, "utf8");
    const reference = code.match(/[#@] sourceMappingURL=([^\s]+)/)?.[1];
    const mapPath = reference ? path.resolve(path.dirname(target), reference) : target + ".map";
    let map;
    try {
      map = AnyMap(JSON.parse(await fs.readFile(mapPath, "utf8")), pathToFileURL(mapPath).href);
    } catch (error) {
      if (error.code === "ENOENT") continue;
      throw error;
    }
    for (const match of code.matchAll(/_s_[a-f0-9]+/g)) {
      const before = code.slice(0, match.index);
      const mapped = originalPositionFor(map, {
        line: before.split("\n").length,
        column: match.index - before.lastIndexOf("\n") - 1,
      });
      const matchesSource = mapped.source
        ?.replaceAll("\\", "/")
        .split(/[?#]/)[0]
        .endsWith(sourcePath);
      if (matchesSource && mapped.line === expected.line && mapped.column === expected.column) {
        return { source: sourcePath, line: mapped.line, column: mapped.column };
      }
      if (candidates.length < 12) candidates.push({ file, ...mapped });
    }
  }
  assert.fail(
    `No generated class maps to ${sourcePath}:${expected.line}:${expected.column}: ${JSON.stringify(candidates)}`,
  );
}
