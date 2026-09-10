import { GenMapping, addSegment, setSourceContent, toEncodedMap } from "@jridgewell/gen-mapping";

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
