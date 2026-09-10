import { isAbsolute, resolve } from "node:path";
import { pathToFileURL } from "node:url";

import { GenMapping, addSegment, setSourceContent, toEncodedMap } from "@jridgewell/gen-mapping";
import remapping from "@jridgewell/remapping";

export interface CssUpstreamMap {
  id: string;
  map: string;
}

function composeUpstream(map: ReturnType<typeof toEncodedMap>, upstream?: CssUpstreamMap) {
  if (!upstream) return map;
  const resource = pathToFileURL(resolve(upstream.id)).href;
  const original = JSON.parse(upstream.map);
  if (!Array.isArray(original.sources) || !original.mappings) return map;
  const sourceUrl = (source: string, base: string) => {
    const normalized = source.replaceAll("\\", "/");
    const clean = normalized.replace(/[?#].*$/, "");
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
    context.depth === 1 && source === resource ? original : null,
  );
}

export interface CssLocations {
  sources: Array<{ path: string; content: string }>;
  entries: Array<{
    source: number;
    generatedStart: number;
    generatedEnd: number;
    originalStart: number;
    originalEnd: number;
    exact: boolean;
  }>;
}

/** Rewrite resource placeholders while retaining native UTF-8 source locations. */
export function cssWithSourceMap(
  output: { css: string; sourceMap: CssLocations },
  replacements: ReadonlyMap<string, string>,
  root: string,
  upstream?: CssUpstreamMap,
  includeSourcesContent = true,
) {
  if (!output.css) return "";
  const { css, sourceMap } = output;
  const positions = sourceMap.sources.map(({ content }) => {
    const result = new Map<number, [number, number]>();
    let byte = 0,
      line = 0,
      column = 0;
    for (const char of content) {
      result.set(byte, [line, column]);
      byte += Buffer.byteLength(char);
      if (char === "\n") {
        line++;
        column = 0;
      } else column += char.length;
    }
    result.set(byte, [line, column]);
    return result;
  });
  const names = sourceMap.sources.map(({ path }) => pathToFileURL(resolve(root, path)).href);
  const map = new GenMapping();
  names.forEach((name, index) => setSourceContent(map, name, sourceMap.sources[index].content));
  let result = "",
    byte = 0,
    line = 0,
    column = 0,
    entryIndex = 0;
  const entries = [...sourceMap.entries].sort((a, b) => a.generatedStart - b.generatedStart);
  const edits = [...replacements];
  for (let offset = 0; offset < css.length;) {
    while (entryIndex < entries.length && entries[entryIndex].generatedEnd <= byte) entryIndex++;
    const entry = entries[entryIndex];
    const active = entry && entry.generatedStart <= byte ? entry : undefined;
    const replacement = edits.find(([placeholder]) => css.startsWith(placeholder, offset));
    const originalText = replacement?.[0] ?? String.fromCodePoint(css.codePointAt(offset)!);
    const emitted = replacement?.[1] ?? originalText;
    const position =
      active &&
      positions[active.source].get(
        active.originalStart + (active.exact ? byte - active.generatedStart : 0),
      );
    for (const char of emitted) {
      if (active && position) addSegment(map, line, column, names[active.source], ...position);
      else addSegment(map, line, column);
      if (char === "\n") {
        line++;
        column = 0;
      } else column += char.length;
    }
    result += emitted;
    byte += Buffer.byteLength(originalText);
    offset += originalText.length;
  }
  return (
    result +
    "\n/*# sourceMappingURL=data:application/json;base64," +
    Buffer.from(
      JSON.stringify(composeUpstream(toEncodedMap(map), upstream), (key, value) =>
        key === "sourcesContent" && !includeSourcesContent ? undefined : value,
      ),
    ).toString("base64") +
    " */"
  );
}

/** Self-contained carriers survive independent processes and filesystem caches. */
export function decodeCssSourceMap(css: string) {
  const match =
    /\n\/\*# sourceMappingURL=data:application\/json;base64,([A-Za-z0-9+/=]+) \*\/$/.exec(css);
  if (!match) return { code: css, map: null };
  return {
    code: css.slice(0, match.index),
    map: JSON.parse(Buffer.from(match[1], "base64").toString("utf8")),
  };
}
