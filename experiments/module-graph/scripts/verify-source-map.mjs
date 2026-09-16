import assert from "node:assert/strict";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { AnyMap, originalPositionFor } from "@jridgewell/trace-mapping";

// Rspack's default module filename template emits project-relative webpack URLs.
// TraceMap may remove their ./ prefix, and duplicate source names gain a query hash.
export function resolveSourceMapFile(source, mapFile, projectRoot) {
  const name = source.split(/[?#]/)[0];
  if (name.startsWith("webpack:///")) {
    return path.resolve(projectRoot, decodeURIComponent(name.slice("webpack:///".length)));
  }
  if (name.startsWith("file:")) return fileURLToPath(name);
  return path.resolve(path.dirname(mapFile), name);
}

export async function verifyMacroSourceMap(
  directory,
  sourcePath,
  source,
  macro,
  css = false,
  knownIncorrectSource,
) {
  const originalOffset = source.indexOf(macro);
  assert.ok(originalOffset >= 0);
  const prefix = source.slice(0, originalOffset);
  const expected = {
    line: prefix.split("\n").length,
    column: originalOffset - prefix.lastIndexOf("\n") - 1,
  };
  const candidates = [];
  for (const file of await fs.readdir(directory, { recursive: true })) {
    if (!(css ? /\.css$/ : /\.[cm]?js$/).test(file)) continue;
    const target = path.join(directory, file);
    const code = await fs.readFile(target, "utf8");
    const reference = code.match(/[#@] sourceMappingURL=([^\s*]+)/)?.[1];
    const mapPath = reference ? path.resolve(path.dirname(target), reference) : target + ".map";
    let map;
    try {
      map = AnyMap(JSON.parse(await fs.readFile(mapPath, "utf8")), pathToFileURL(mapPath).href);
    } catch (error) {
      if (error.code === "ENOENT") continue;
      throw error;
    }
    for (const match of code.matchAll(/sw_[0-9a-z]+/g)) {
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
  // Record this narrowly observed pipeline failure without treating it as map support.
  const knownMismatch =
    knownIncorrectSource &&
    candidates.find(
      (candidate) =>
        candidate.source?.endsWith(knownIncorrectSource) &&
        candidate.line === expected.line &&
        candidate.column === expected.column,
    );
  if (knownMismatch)
    return {
      supported: false,
      issue: "css-source-mismatch",
      expected: { source: sourcePath, ...expected },
      actual: knownMismatch,
    };
  assert.fail(
    `No generated class maps to ${sourcePath}:${expected.line}:${expected.column}: ${JSON.stringify(candidates)}`,
  );
}
