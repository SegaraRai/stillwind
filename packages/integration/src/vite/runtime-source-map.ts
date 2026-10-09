import { GenMapping, addSegment, toEncodedMap } from "@jridgewell/gen-mapping";
import { TraceMap, eachMapping } from "@jridgewell/trace-mapping";

import type { StylesheetPart } from "./runtime.ts";

/**
 * Concatenates ordered CSS parts and combines their mappings without embedding original sources.
 * @param parts - Partitions in stylesheet order, each with an optional map for its own CSS text.
 * @returns Combined CSS followed by a UTF-8, base64-encoded inline version 3 map.
 * @throws If a supplied source map cannot be decoded.
 */
export function renderCssWithSourceMap(parts: readonly StylesheetPart[]): string {
  const map = new GenMapping();
  let line = 0;
  let column = 0;
  for (const part of parts) {
    if (part.map) {
      const trace = new TraceMap(part.map);
      eachMapping(trace, (entry) => {
        const generatedLine = line + entry.generatedLine - 1;
        const generatedColumn = (entry.generatedLine === 1 ? column : 0) + entry.generatedColumn;
        if (entry.source !== null && entry.originalLine !== null) {
          addSegment(
            map,
            generatedLine,
            generatedColumn,
            entry.source,
            entry.originalLine - 1,
            entry.originalColumn,
          );
        } else {
          addSegment(map, generatedLine, generatedColumn);
        }
      });
    }
    const lines = part.css.split("\n");
    if (lines.length > 1) {
      line += lines.length - 1;
      column = lines.at(-1)!.length;
    } else {
      column += part.css.length;
    }
  }
  const encodedMap = toEncodedMap(map);
  delete encodedMap.sourcesContent;
  const bytes = new TextEncoder().encode(JSON.stringify(encodedMap));
  const encoded = btoa(Array.from(bytes, (byte) => String.fromCharCode(byte)).join(""));
  return `${parts.map((part) => part.css).join("")}\n/*# sourceMappingURL=data:application/json;base64,${encoded} */`;
}

/**
 * Replaces recorded placeholders and shifts an inline map to the resulting CSS positions.
 * @param css - Generated CSS with an optional trailing inline map in the format emitted by this module.
 * @param replacements - Placeholder/replacement pairs. First matching pair wins at each original position.
 * @returns Replaced CSS with adjusted mappings when an inline map was present. Replacement text is not rescanned.
 * @throws If a recognized inline map contains invalid base64, JSON, or source-map data.
 */
export function replaceCssResources(css: string, replacements: [string, string][]): string {
  const match =
    /\n\/\*# sourceMappingURL=data:application\/json;base64,([A-Za-z0-9+/=]+) \*\/$/.exec(css);
  const text = match ? css.slice(0, match.index) : css;
  const edits: {
    /** Inclusive UTF-16 placeholder offset in CSS before replacement. */
    from: number;
    /** Exclusive UTF-16 placeholder offset in that original CSS. */
    to: number;
    /** Replacement contents whose UTF-16 length shifts subsequent mappings. */
    text: string;
    /** Inclusive UTF-16 replacement offset in the resulting CSS. */
    start: number;
  }[] = [];
  let result = "";
  for (let offset = 0; offset < text.length;) {
    const replacement = replacements.find(([placeholder]) => text.startsWith(placeholder, offset));
    if (replacement?.[0].length) {
      edits.push({
        from: offset,
        to: offset + replacement[0].length,
        text: replacement[1],
        start: result.length,
      });
      result += replacement[1];
      offset += replacement[0].length;
    } else {
      result += text[offset++];
    }
  }
  if (!match) {
    return result;
  }
  const bytes = Uint8Array.from(atob(match[1]), (character) => character.charCodeAt(0));
  const original = new TraceMap(JSON.parse(new TextDecoder().decode(bytes)));
  const starts = [0];
  for (const found of text.matchAll(/\n/g)) {
    starts.push(found.index + 1);
  }
  const map = new GenMapping();
  const resultStarts = [0];
  for (const found of result.matchAll(/\n/g)) {
    resultStarts.push(found.index + 1);
  }
  eachMapping(original, (entry) => {
    const offset = starts[entry.generatedLine - 1] + entry.generatedColumn;
    let transformed = offset;
    for (const edit of edits) {
      if (offset >= edit.to) {
        transformed += edit.text.length - (edit.to - edit.from);
      } else if (offset >= edit.from) {
        transformed = edit.start;
        break;
      } else {
        break;
      }
    }
    let left = 0;
    let right = resultStarts.length;
    while (left + 1 < right) {
      const middle = Math.floor((left + right) / 2);
      if (resultStarts[middle] <= transformed) {
        left = middle;
      } else {
        right = middle;
      }
    }
    const line = left;
    const column = transformed - resultStarts[line];
    if (entry.source !== null && entry.originalLine !== null) {
      addSegment(map, line, column, entry.source, entry.originalLine - 1, entry.originalColumn);
    } else {
      addSegment(map, line, column);
    }
  });
  const encoded = toEncodedMap(map);
  return renderCssWithSourceMap([
    {
      producers: [],
      css: result,
      map: {
        version: 3,
        names: [],
        sources: [...encoded.sources],
        mappings: encoded.mappings,
      },
    },
  ]);
}
