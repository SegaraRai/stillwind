import { GenMapping, addSegment, setSourceContent, toEncodedMap } from "@jridgewell/gen-mapping";
import { TraceMap, eachMapping } from "@jridgewell/trace-mapping";

import type { StylesheetPart } from "./runtime.ts";

/** Returns the ordered CSS parts with a combined inline source map. */
export function renderCssWithSourceMap(parts: StylesheetPart[]): string {
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
      trace.sources.forEach((source, index) => {
        if (source !== null && trace.sourcesContent?.[index] != null) {
          setSourceContent(map, source, trace.sourcesContent[index]);
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
  const bytes = new TextEncoder().encode(JSON.stringify(toEncodedMap(map)));
  const encoded = btoa(Array.from(bytes, (byte) => String.fromCharCode(byte)).join(""));
  return `${parts.map((part) => part.css).join("")}\n/*# sourceMappingURL=data:application/json;base64,${encoded} */`;
}
