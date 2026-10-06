import { isAbsolute, resolve } from "node:path";
import { pathToFileURL } from "node:url";

import {
  GenMapping,
  addSegment,
  setSourceContent,
  toEncodedMap,
  type EncodedSourceMap,
} from "@jridgewell/gen-mapping";
import remapping, { type SourceMap } from "@jridgewell/remapping";
import type { EncodedSourceMap as MutableEncodedSourceMap } from "@jridgewell/trace-mapping";

/**
 * Producer map used to trace native CSS locations from a transformed script or component back to
 * its authored source.
 */
export interface CssUpstreamMap {
  /**
   * Producer filesystem identity used as the base for relative map sources. Ordinary query and
   * fragment suffixes may follow its path.
   */
  id: string;
  /** Exact source name in the native CSS map to which this producer map applies. */
  sourceMapSource: string;
  /** JSON text for an encoded version 3 map from producer input to authored sources. */
  map: string;
}

/**
 * Composes one producer map into the matching immediate source of a CSS map.
 *
 * Relative producer sources are resolved against its resource path and optional source root. Other
 * CSS sources keep their existing mappings.
 *
 * @param map - Encoded CSS map whose source names identify original producer inputs.
 * @param upstream - Optional producer map and the exact CSS source name to replace.
 * @returns The composed map, or the input map unchanged when upstream is absent or lacks a sources
 * array or nonempty mappings.
 * @throws If producer JSON, source URLs, or source-map composition are invalid.
 */
function composeUpstream(
  map: EncodedSourceMap,
  upstream?: CssUpstreamMap,
): EncodedSourceMap | SourceMap {
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

/**
 * Native locations linking generated CSS intervals to authored text. All interval offsets count
 * UTF-8 bytes before resource placeholders are replaced.
 */
export interface CssLocations {
  /** Source table indexed by each entry's `source` field. */
  sources: {
    /** Source identity recorded in the emitted version 3 map. */
    sourceMapSource: string;
    /**
     * Authored source text used to derive original lines, UTF-16 columns, and optional
     * `sourcesContent`.
     */
    content: string;
  }[];
  /**
   * Nonoverlapping generated CSS intervals. Rendering sorts them by generated start. Callers must
   * supply valid source indexes and byte boundaries.
   */
  entries: {
    /** Zero-based index into `sources` for the authored interval. */
    source: number;
    /** Inclusive UTF-8 byte offset in generated CSS before resource replacement. */
    generatedStart: number;
    /** Exclusive UTF-8 byte offset in that CSS, at or after `generatedStart`. */
    generatedEnd: number;
    /** Inclusive UTF-8 byte offset in the selected source's content. */
    originalStart: number;
    /** Exclusive UTF-8 byte offset in that content, at or after `originalStart`. */
    originalEnd: number;
    /**
     * True maps matching text byte for byte from `originalStart`. False maps the generated interval
     * to its original start.
     */
    exact: boolean;
  }[];
}

/** CSS and native source locations before resource URLs and upstream maps are composed. */
export interface CssSourceMapInput {
  /** Stylesheet text containing any unresolved resource placeholder tokens. */
  css: string;
  /** Native source locations whose generated offsets count UTF-8 bytes in `css`. */
  sourceMap: CssLocations;
}

/**
 * Rewrites resource placeholders while retaining native UTF-8 source locations.
 *
 * @param output - CSS and native source locations before resource replacement. Source indexes and
 * byte boundaries must be valid for the supplied CSS and authored text.
 * @param replacements - Resource placeholders mapped to escaped URL text.
 * @param upstream - Optional maps from producer input back to its original source, in composition order.
 * @param includeSourcesContent - Whether the inline map retains source text. Defaults to true.
 * @returns Resolved CSS with an inline version 3 map, or an empty string when the input CSS is empty.
 * @throws If an upstream map cannot be decoded, its source URLs are invalid, or composition fails.
 */
export function cssWithSourceMap(
  output: CssSourceMapInput,
  replacements: ReadonlyMap<string, string>,
  upstream?: CssUpstreamMap | readonly CssUpstreamMap[],
  includeSourcesContent = true,
) {
  if (!output.css) {
    return "";
  }
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
      } else {
        column += char.length;
      }
    }
    result.set(byte, [line, column]);
    return result;
  });
  const names = sourceMap.sources.map(({ sourceMapSource }) => sourceMapSource);
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
    while (entryIndex < entries.length && entries[entryIndex].generatedEnd <= byte) {
      entryIndex++;
    }
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

/** Stylesheet text and its decoded inline source map, ready for a bundler's CSS pipeline. */
export interface DecodedCssSourceMap {
  /** CSS with the recognized trailing inline source-map comment removed. */
  code: string;
  /**
   * Decoded version 3 map with mutable arrays and string source identities, or null when the CSS
   * has no recognized trailing map.
   */
  map: (MutableEncodedSourceMap & { sources: string[] }) | null;
}

/**
 * Decodes the trailing inline map emitted by `cssWithSourceMap` or a compatible CSS carrier.
 *
 * @param css - Stylesheet text. A recognized map must contain a base64-encoded version 3 JSON map
 * in the trailing comment format emitted by `cssWithSourceMap`.
 * @returns CSS and its decoded map. Without a recognized trailing map, the CSS is unchanged and
 * the map is null. JSON is decoded without validating its source-map fields.
 * @throws If a recognized inline map contains invalid JSON.
 */
export function decodeCssSourceMap(css: string): DecodedCssSourceMap {
  const match =
    /\n\/\*# sourceMappingURL=data:application\/json;base64,([A-Za-z0-9+/=]+) \*\/$/.exec(css);
  if (!match) {
    return {
      code: css,
      map: null,
    };
  }
  return {
    code: css.slice(0, match.index),
    map: JSON.parse(Buffer.from(match[1], "base64").toString("utf8")) as NonNullable<
      DecodedCssSourceMap["map"]
    >,
  };
}
