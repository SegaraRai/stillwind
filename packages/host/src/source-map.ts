import { resolve } from "node:path";

import { GenMapping, addSegment, setSourceContent, toEncodedMap } from "@jridgewell/gen-mapping";
import remapping from "@jridgewell/remapping";
import { TraceMap } from "@jridgewell/trace-mapping";

/** Repair framework loaders that publish maps to the transformed SFC input. */
export function composeFrameworkSourceMap(
  generated: Exclude<Parameters<typeof remapping>[0], unknown[]>,
  original: Parameters<typeof remapping>[0],
  transformedSource: string,
) {
  const traced = new TraceMap(generated);
  const matches = new Set(
    traced.resolvedSources.filter(
      (_source, index) => traced.sourcesContent?.[index] === transformedSource,
    ),
  );
  return remapping(generated, (source, context) =>
    context.depth === 1 && matches.has(source)
      ? (original as Exclude<Parameters<typeof remapping>[0], unknown[]>)
      : null,
  );
}

export function composeCarrierSourceMap(
  generated: Parameters<typeof remapping>[0],
  original: Parameters<typeof remapping>[0],
  resource: string,
) {
  return remapping(generated, (source, context) =>
    context.depth === 1 && resolve(source).replaceAll("\\", "/") === resource
      ? (original as Exclude<Parameters<typeof remapping>[0], unknown[]>)
      : null,
  );
}

/** The upstream loader map describes the input that native compilation received. */
export function composeModuleSourceMap(
  generated: ReturnType<typeof moduleSourceMap>,
  upstream: Parameters<typeof remapping>[0] | undefined | null,
) {
  return upstream
    ? remapping(
        [JSON.stringify(generated), ...(Array.isArray(upstream) ? upstream : [upstream])],
        () => null,
      )
    : generated;
}

export interface CompilerMapping {
  original_start: number;
  original_end: number;
  generated_start: number;
  generated_end: number;
}

/** Compose native UTF-16 edits, removed macro imports and synthetic host imports. */
export function moduleSourceMap(
  source: string,
  compiled: string,
  id: string,
  mappings: CompilerMapping[],
  removed: Array<{ from: number; to: number }>,
  prefix: string,
) {
  const map = new GenMapping({ file: id });
  setSourceContent(map, id, source);
  const lines = [0];
  for (let index = 0; index < source.length; index++) {
    if (source[index] === "\n") lines.push(index + 1);
  }
  let line = prefix.split("\n").length - 1;
  let column = prefix.length - prefix.lastIndexOf("\n") - 1;
  let editIndex = 0;
  let removedIndex = 0;
  let delta = 0;
  let originalLine = 0;
  for (let index = 0; index < compiled.length; index++) {
    while (removedIndex < removed.length && removed[removedIndex].to <= index) removedIndex++;
    if (removedIndex < removed.length && removed[removedIndex].from <= index) continue;
    while (editIndex < mappings.length && mappings[editIndex].generated_end <= index) {
      const edit = mappings[editIndex++];
      delta = edit.original_end - edit.generated_end;
    }
    const edit = mappings[editIndex];
    const original = edit && edit.generated_start <= index ? edit.original_start : index + delta;
    while (originalLine + 1 < lines.length && lines[originalLine + 1] <= original) originalLine++;
    addSegment(map, line, column, id, originalLine, original - lines[originalLine]);
    if (compiled[index] === "\n") {
      line++;
      column = 0;
    } else column++;
  }
  return toEncodedMap(map);
}
