import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

import {
  GenMapping,
  addSegment,
  setSourceContent,
  toEncodedMap,
  type EncodedSourceMap,
} from "@jridgewell/gen-mapping";
import remapping, { type SourceMap } from "@jridgewell/remapping";
import { TraceMap, type SourceMapInput } from "@jridgewell/trace-mapping";

/**
 * Traces framework output through a Stillwind transform when embedded source text identifies the
 * transformed input.
 *
 * @param generated - Framework output map.
 * @param original - Map or map chain from the Stillwind-transformed input back to original sources.
 * @param transformedSource - Exact text to match against the generated map's `sourcesContent`.
 * @returns A composed map. Only matching immediate sources are replaced. Other sources retain their
 * mappings.
 * @throws If a supplied source map cannot be decoded or composed.
 */
export function composeFrameworkSourceMap(
  generated: SourceMapInput,
  original: SourceMapInput | readonly SourceMapInput[],
  transformedSource: string,
): SourceMap {
  const traced = new TraceMap(generated);
  const matches = new Set(
    traced.resolvedSources.filter(
      (_source, index) => traced.sourcesContent?.[index] === transformedSource,
    ),
  );
  return remapping(generated, (source, context) =>
    context.depth === 1 && matches.has(source) ? (original as SourceMapInput) : null,
  );
}

/**
 * Traces a CSS pipeline map through the original map of its carrier resource.
 *
 * @param generated - Pipeline output map, or a map chain accepted by the composition library.
 * @param original - Map or chain from the CSS carrier back to authored sources.
 * @param resource - Absolute carrier path with forward slashes. File URLs are decoded, and relative
 * source paths are resolved against the process working directory before comparison.
 * @returns A composed map that replaces only immediate sources resolving to the carrier path.
 * @throws If a supplied source map cannot be decoded or composed.
 */
export function composeCarrierSourceMap(
  generated: SourceMapInput | SourceMapInput[],
  original: SourceMapInput | readonly SourceMapInput[],
  resource: string,
): SourceMap {
  return remapping(generated, (source, context) =>
    context.depth === 1 &&
    resolve(source.startsWith("file:") ? fileURLToPath(source) : source).replaceAll("\\", "/") ===
      resource
      ? (original as SourceMapInput)
      : null,
  );
}

/**
 * Map or ordered map chain from a module's input back to its original sources. Null and undefined
 * mean that no upstream map is available. Each map may be serialized or supplied as an object.
 */
export type UpstreamSourceMap = SourceMapInput | readonly SourceMapInput[] | undefined | null;

/**
 * Composes the native output map with the upstream loader map or ordered map chain
 * describing its input. Without upstream, returns the generated object unchanged.
 * Invalid maps or incompatible chains may throw from the mapping library.
 *
 * @param generated - Map from compiled JavaScript back to the input received by Stillwind.
 * @param upstream - Optional map or ordered map chain from that input back to original sources.
 * @returns The generated map unchanged when no upstream map is supplied, otherwise a composed map.
 * @throws If map decoding fails or a supplied map chain is incompatible.
 */
export function composeModuleSourceMap(
  generated: EncodedSourceMap,
  upstream: UpstreamSourceMap,
): EncodedSourceMap | SourceMap {
  return upstream
    ? remapping(
        [JSON.stringify(generated), ...(Array.isArray(upstream) ? upstream : [upstream])],
        () => null,
      )
    : generated;
}

/**
 * One compiler replacement before host finalization inserts a prefix or removes text.
 * All offsets are absolute UTF-16 code units in the original and compiled buffers.
 * Replacement gaps without expression intervals map to `originalStart`.
 */
export interface CompilerMapping {
  /** Inclusive original offset. Equals end for insertion. */
  originalStart: number;
  /** Exclusive original offset, at or after start. */
  originalEnd: number;
  /** Inclusive offset in the complete compiled buffer. */
  generatedStart: number;
  /** Exclusive compiled offset. Equals start for deletion. */
  generatedEnd: number;
  /**
   * Nonempty, nonoverlapping intervals contained in this replacement, sorted by
   * generated start. Source order may differ. Missing/empty means anchor-only mapping.
   */
  expressions?: Array<{
    /** Inclusive original offset. The anchor for a nonexact interval. */
    originalStart: number;
    /** Exclusive original offset of the expression or encoded character. */
    originalEnd: number;
    /** Inclusive compiled offset, before host edits. */
    generatedStart: number;
    /** Exclusive compiled offset, strictly greater than start. */
    generatedEnd: number;
    /** True requires identical text/equal lengths. False anchors all units at original start. */
    exact: boolean;
  }>;
}

/**
 * Builds a JavaScript source map from compiler replacements and host finalization edits.
 *
 * Unchanged text maps character by character. Replacement text maps to its original start unless an
 * expression interval provides a more precise correspondence. All offsets use UTF-16 code units.
 *
 * @param source - Original source whose text is embedded in the map.
 * @param compiled - Compiler output before the host inserts a prefix or removes intervals.
 * @param id - Source identity used as the map's file and sole source name.
 * @param mappings - Nonoverlapping compiler replacements in generated-start order, with valid
 * original and generated ranges.
 * @param removed - Nonoverlapping ranges removed from `compiled`, in ascending order.
 * @param prefix - Exact text inserted before the retained compiler output. It advances generated
 * positions without receiving original mappings.
 * @returns An encoded version 3 map for the finalized output with original source content.
 */
export function moduleSourceMap(
  source: string,
  compiled: string,
  id: string,
  mappings: readonly CompilerMapping[],
  removed: readonly {
    /** Inclusive UTF-16 offset removed from compiled. */
    from: number;
    /** Exclusive UTF-16 offset in the same compiled buffer. */
    to: number;
  }[],
  prefix: string,
): EncodedSourceMap {
  const map = new GenMapping({ file: id });
  setSourceContent(map, id, source);
  const lines = [0];
  for (let index = 0; index < source.length; index++) {
    if (source[index] === "\n") {
      lines.push(index + 1);
    }
  }
  let line = prefix.split("\n").length - 1;
  let column = prefix.length - prefix.lastIndexOf("\n") - 1;
  let editIndex = 0;
  let removedIndex = 0;
  let expressionIndex = 0;
  let delta = 0;
  let originalLine = 0;
  for (let index = 0; index < compiled.length; index++) {
    while (removedIndex < removed.length && removed[removedIndex].to <= index) {
      removedIndex++;
    }
    if (removedIndex < removed.length && removed[removedIndex].from <= index) {
      continue;
    }
    while (editIndex < mappings.length && mappings[editIndex].generatedEnd <= index) {
      const edit = mappings[editIndex++];
      delta = edit.originalEnd - edit.generatedEnd;
      expressionIndex = 0;
    }
    const edit = mappings[editIndex];
    const expressions = edit?.expressions ?? [];
    while (
      expressionIndex < expressions.length &&
      expressions[expressionIndex].generatedEnd <= index
    ) {
      expressionIndex++;
    }
    const expression = expressions[expressionIndex];
    const original =
      expression && expression.generatedStart <= index
        ? expression.originalStart + (expression.exact ? index - expression.generatedStart : 0)
        : edit && edit.generatedStart <= index
          ? edit.originalStart
          : index + delta;
    // A folded record can jump backwards between distant source lines.
    if (
      original < lines[originalLine] ||
      (originalLine + 1 < lines.length && lines[originalLine + 1] <= original)
    ) {
      let low = 0;
      let high = lines.length;
      while (low < high) {
        const middle = (low + high) >>> 1;
        if (lines[middle] <= original) {
          low = middle + 1;
        } else {
          high = middle;
        }
      }
      originalLine = low - 1;
    }
    addSegment(map, line, column, id, originalLine, original - lines[originalLine]);
    if (compiled[index] === "\n") {
      line++;
      column = 0;
    } else {
      column++;
    }
  }
  return toEncodedMap(map);
}
