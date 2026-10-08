import { isAbsolute, resolve } from "node:path";
import { pathToFileURL } from "node:url";

import {
  GenMapping,
  addSegment,
  setSourceContent,
  toEncodedMap,
  type EncodedSourceMap,
} from "@jridgewell/gen-mapping";
import remapping from "@jridgewell/remapping";

export interface CssUpstreamMap {
  id: string;
  sourceMapSource: string;
  map: string;
}

function composeUpstream(map: EncodedSourceMap, upstream?: CssUpstreamMap) {
  if (!upstream) {
    return map;
  }
  const resource = pathToFileURL(resolve(upstream.id)).href;
  const original = JSON.parse(upstream.map);
  if (!Array.isArray(original.sources) || !original.mappings) {
    return map;
  }
  const sourceUrl = (source: string, base: string) => {
    const normalized = source.replaceAll("\\", "/");
    const clean = normalized.split(/[?#]/)[0];
    return isAbsolute(clean)
      ? pathToFileURL(clean).href + normalized.slice(clean.length)
      : new URL(normalized, base).href;
  };
  const base = original.sourceRoot
    ? sourceUrl(original.sourceRoot.replace(/\/?$/, "/"), resource)
    : resource;
  original.sources = original.sources.map((source: string) => sourceUrl(source, base));
  delete original.sourceRoot;
  return remapping(JSON.stringify(map), (source, context) =>
    context.depth === 1 && source === upstream.sourceMapSource ? original : null,
  );
}

export interface CssLocations {
  sources: Array<{ sourceMapSource: string; content: string }>;
  entries: Array<{
    source: number;
    generatedStart: number;
    generatedEnd: number;
    originalStart: number;
    originalEnd: number;
    exact: boolean;
  }>;
}

// Only index mapping starts. A cursor then advances through exact source text.
// Sorted requests scan each source once; no per-character Map or tuple allocation.
function entryPositions(sourceMap: CssLocations) {
  const result = new Map<CssLocations["entries"][number], { at(byte: number): [number, number] }>();
  const groups = sourceMap.sources.map(() => [] as CssLocations["entries"]);
  for (const entry of sourceMap.entries) {
    groups[entry.source].push(entry);
  }
  sourceMap.sources.forEach(({ content }, source) => {
    let byte = 0;
    let offset = 0;
    let line = 0;
    let column = 0;
    const entries = groups[source].sort((a, b) => a.originalStart - b.originalStart);
    for (const entry of entries) {
      while (byte < entry.originalStart) {
        const char = String.fromCodePoint(content.codePointAt(offset)!);
        byte += Buffer.byteLength(char);
        offset += char.length;
        if (char === "\n") {
          line++;
          column = 0;
        } else {
          column += char.length;
        }
      }
      let cursorByte = byte;
      let cursorOffset = offset;
      let cursorLine = line;
      let cursorColumn = column;
      result.set(entry, {
        at(target) {
          while (cursorByte < target) {
            const char = String.fromCodePoint(content.codePointAt(cursorOffset)!);
            cursorByte += Buffer.byteLength(char);
            cursorOffset += char.length;
            if (char === "\n") {
              cursorLine++;
              cursorColumn = 0;
            } else {
              cursorColumn += char.length;
            }
          }
          return [cursorLine, cursorColumn];
        },
      });
    }
  });
  return result;
}

/** Rewrite resource placeholders while retaining native UTF-8 source locations. */
export function cssWithSourceMap(
  output: { css: string; sourceMap: CssLocations },
  replacements: ReadonlyMap<string, string>,
  upstream?: CssUpstreamMap | readonly CssUpstreamMap[],
  includeSourcesContent = true,
) {
  if (!output.css) {
    return "";
  }
  const { css, sourceMap } = output;
  const positions = entryPositions(sourceMap);
  const names = sourceMap.sources.map(({ sourceMapSource }) => sourceMapSource);
  const map = new GenMapping();
  names.forEach((name, index) => setSourceContent(map, name, sourceMap.sources[index].content));
  let result = "";
  let byte = 0;
  let line = 0;
  let column = 0;
  let entryIndex = 0;
  const entries = [...sourceMap.entries].sort((a, b) => a.generatedStart - b.generatedStart);
  const keys = [...replacements.keys()];
  const compilerKeys = keys.every((key) =>
    /^__stillwind_(?:js_)?resource_[a-f0-9]{64}__$/.test(key),
  );
  const pattern = replacements.size
    ? compilerKeys
      ? /__stillwind_(?:js_)?resource_[a-f0-9]{64}__/gu
      : new RegExp(
          keys
            .map((key) => {
              if (!key) {
                throw new Error("empty placeholder");
              }
              return key.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
            })
            .join("|"),
          "gu",
        )
    : null;
  let match = pattern?.exec(css);
  for (let offset = 0; offset < css.length;) {
    while (entryIndex < entries.length && entries[entryIndex].generatedEnd <= byte) {
      entryIndex++;
    }
    const entry = entries[entryIndex];
    const active = entry && entry.generatedStart <= byte ? entry : undefined;
    while (match && match.index < offset) {
      match = pattern!.exec(css);
    }
    const replacement =
      match?.index === offset && replacements.has(match[0])
        ? [match[0], replacements.get(match[0])!]
        : undefined;
    const originalText = replacement?.[0] ?? String.fromCodePoint(css.codePointAt(offset)!);
    const emitted = replacement?.[1] ?? originalText;
    const position =
      active &&
      positions
        .get(active)!
        .at(active.originalStart + (active.exact ? byte - active.generatedStart : 0));
    for (const char of emitted) {
      if (active && position) {
        addSegment(map, line, column, names[active.source], ...position);
      } else {
        addSegment(map, line, column);
      }
      if (char === "\n") {
        line++;
        column = 0;
      } else {
        column += char.length;
      }
    }
    result += emitted;
    byte += Buffer.byteLength(originalText);
    offset += originalText.length;
  }
  return (
    result +
    "\n/*# sourceMappingURL=data:application/json;base64," +
    Buffer.from(
      JSON.stringify(
        (Array.isArray(upstream) ? upstream : [upstream]).reduce(
          (map, source) => composeUpstream(map, source),
          toEncodedMap(map),
        ),
        (key, value) => (key === "sourcesContent" && !includeSourcesContent ? undefined : value),
      ),
    ).toString("base64") +
    " */"
  );
}

/** Self-contained carriers survive independent processes and filesystem caches. */
export function decodeCssSourceMap(css: string) {
  const match =
    /\n\/\*# sourceMappingURL=data:application\/json;base64,([A-Za-z0-9+/=]+) \*\/$/.exec(css);
  if (!match) {
    return { code: css, map: null };
  }
  return {
    code: css.slice(0, match.index),
    map: JSON.parse(Buffer.from(match[1], "base64").toString("utf8")),
  };
}
