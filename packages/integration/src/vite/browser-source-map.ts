import {
  GenMapping,
  addSegment,
  toEncodedMap,
  type EncodedSourceMap,
} from "@jridgewell/gen-mapping";
import { TraceMap, eachMapping } from "@jridgewell/trace-mapping";

/** Serialized or object source map accepted by the tracing library, including readonly arrays. */
export type BrowserSourceMapInput = ConstructorParameters<typeof TraceMap>[0];

/**
 * Publishes positions with caller-approved source names and no source text or symbol names.
 *
 * @param input - Serialized or object source map accepted by the tracing library.
 * @param sourceName - Maps each original source name to the name approved for browser delivery.
 * @returns An encoded map retaining positions with the approved names and no embedded source text.
 * @throws If the input is not a valid source map.
 */
export function browserSourceMap(
  input: BrowserSourceMapInput,
  sourceName: (source: string) => string,
): EncodedSourceMap {
  const trace = new TraceMap(input);
  const map = new GenMapping();
  const sources = new Map<string, string>();
  eachMapping(trace, (entry) => {
    if (entry.source !== null && entry.originalLine !== null) {
      let source = sources.get(entry.source);
      if (source === undefined) {
        source = sourceName(entry.source);
        sources.set(entry.source, source);
      }
      addSegment(
        map,
        entry.generatedLine - 1,
        entry.generatedColumn,
        source,
        entry.originalLine - 1,
        entry.originalColumn,
      );
    } else {
      addSegment(map, entry.generatedLine - 1, entry.generatedColumn);
    }
  });
  const result = toEncodedMap(map);
  delete result.sourcesContent;
  return result;
}
