import assert from "node:assert/strict";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { AnyMap, originalPositionFor } from "@jridgewell/trace-mapping";

// Rspack's default module filename template emits project-relative webpack URLs.
// TraceMap may remove their ./ prefix, and duplicate source names gain a query hash.
export function resolveSourceMapFile(source: string, mapFile: string, projectRoot: string) {
  const name = source.split(/[?#]/)[0];
  if (name.startsWith("webpack://")) {
    const location = name.replace(/^webpack:\/\/(?:@[^/]+\/[^/]+|[^/]*)\//, "");
    return path.resolve(projectRoot, decodeURIComponent(location));
  }
  if (name.startsWith("file:")) {
    return fileURLToPath(name);
  }
  return path.resolve(path.dirname(mapFile), name);
}

/**
 * Checks class references in generated files against the given original source and macro.
 * `sourcePath` is the normalized source suffix; `macro` identifies its first matching call.
 * `start` requires the call's first character. `call-span` permits positions inside a JS call,
 * including arguments that the host maps after minification. CSS always requires `start`.
 * Returns the observed position, or unsupported evidence for the declared incorrect source.
 * Throws when no class maps to the requested call or a CSS check requests `call-span`.
 */
export async function verifyMacroSourceMap(
  directory: string,
  sourcePath: string,
  source: string,
  macro: string,
  css = false,
  knownIncorrectSource?: string,
  position: "start" | "call-span" = "start",
) {
  if (css && position !== "start") {
    throw new Error("CSS checks require the start of the macro call");
  }
  const originalOffset =
    source.indexOf(macro) >= 0 ? source.indexOf(macro) : source.indexOf(macro.replaceAll("'", '"'));
  assert.ok(originalOffset >= 0);
  const prefix = source.slice(0, originalOffset);
  const expected = {
    line: prefix.split("\n").length,
    column: originalOffset - prefix.lastIndexOf("\n") - 1,
  };
  const ending = source.slice(0, originalOffset + macro.length);
  const end = {
    line: ending.split("\n").length,
    column: ending.length - ending.lastIndexOf("\n") - 1,
  };
  const candidates = [];
  for (const file of await fs.readdir(directory, { recursive: true })) {
    if (!(css ? /\.css$/ : /\.[cm]?js$/).test(file)) {
      continue;
    }
    const target = path.join(directory, file);
    const code = await fs.readFile(target, "utf8");
    const reference = code.match(/[#@] sourceMappingURL=([^\s*]+)/)?.[1];
    const mapPath = reference ? path.resolve(path.dirname(target), reference) : target + ".map";
    let map;
    try {
      map = AnyMap(JSON.parse(await fs.readFile(mapPath, "utf8")), pathToFileURL(mapPath).href);
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === "ENOENT") {
        continue;
      }
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
      const startsCall = mapped.line === expected.line && mapped.column === expected.column;
      const withinCall =
        mapped.line !== null &&
        mapped.column !== null &&
        (mapped.line > expected.line ||
          (mapped.line === expected.line && mapped.column >= expected.column)) &&
        (mapped.line < end.line || (mapped.line === end.line && mapped.column < end.column));
      if (matchesSource && (position === "call-span" ? withinCall : startsCall)) {
        return { source: sourcePath, line: mapped.line, column: mapped.column, position };
      }
      if (candidates.length < 12) {
        candidates.push({ file, ...mapped });
      }
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
  if (knownMismatch) {
    return {
      supported: false,
      issue: "css-source-mismatch",
      expected: { source: sourcePath, ...expected },
      actual: knownMismatch,
    };
  }
  assert.fail(
    `No generated class maps to ${sourcePath}:${expected.line}:${expected.column}: ${JSON.stringify(candidates)}`,
  );
}
