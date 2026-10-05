import { GenMapping, addSegment, toEncodedMap } from "@jridgewell/gen-mapping";
import { TraceMap, eachMapping } from "@jridgewell/trace-mapping";

/** Publishes positions with caller-approved source names and no source text or symbol names. */
export function browserSourceMap(
  input: ConstructorParameters<typeof TraceMap>[0],
  sourceName: (source: string) => string,
) {
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
