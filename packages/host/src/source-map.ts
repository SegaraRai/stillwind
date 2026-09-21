import { resolve } from "node:path";

import { GenMapping, addSegment, setSourceContent, toEncodedMap } from "@jridgewell/gen-mapping";
import remapping from "@jridgewell/remapping";
import { TraceMap } from "@jridgewell/trace-mapping";

/**
 * Composes original into depth-one sources whose embedded content exactly equals
 * transformedSource. Other sources remain outside this composition step.
 * Invalid maps may throw from the source-map libraries.
 */
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

/**
 * Composes original into depth-one sources resolving to resource. The caller must
 * supply an absolute, forward-slash resource path; source names are resolved against
 * the process working directory. Invalid maps may throw from the mapping libraries.
 */
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

/**
 * Composes the native output map with the upstream loader map or ordered map chain
 * describing its input. Without upstream, returns the generated object unchanged.
 * Invalid maps or incompatible chains may throw from the mapping library.
 */
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

/**
 * One compiler replacement before host prefix insertion or import removal.
 * All offsets are absolute UTF-16 code units in the original and compiled buffers.
 * Replacement gaps without expression intervals map to `original_start`.
 */
export interface CompilerMapping {
  /** Inclusive original offset; equals end for insertion. */
  original_start: number;
  /** Exclusive original offset, at or after start. */
  original_end: number;
  /** Inclusive offset in the complete compiled buffer. */
  generated_start: number;
  /** Exclusive compiled offset; equals start for deletion. */
  generated_end: number;
  /**
   * Nonempty, nonoverlapping intervals contained in this replacement, sorted by
   * generated start. Source order may differ. Missing/empty means anchor-only mapping.
   */
  expressions?: Array<{
    /** Inclusive original offset; the anchor for a nonexact interval. */
    original_start: number;
    /** Exclusive original offset of the expression or encoded character. */
    original_end: number;
    /** Inclusive compiled offset, before host edits. */
    generated_start: number;
    /** Exclusive compiled offset, strictly greater than start. */
    generated_end: number;
    /** True requires identical text/equal lengths; false anchors all units at original start. */
    exact: boolean;
  }>;
}

/**
 * Builds a source-map v3 object for `prefix` followed by `compiled` with `removed` ranges omitted.
 *
 * `source` is the original buffer embedded as sourcesContent under `id`. `mappings`
 * must contain valid, nonoverlapping edits ordered by generated/original replacement
 * start; nested expression intervals follow generated order, not original order.
 * `removed` must be sorted, nonoverlapping half-open UTF-16 ranges in `compiled`.
 * The caller validates these contracts; inputs are not changed.
 *
 * Untouched text and exact expressions map by code-unit offset, encoded expressions
 * use their original anchor, and other replacement syntax maps to the call start.
 * The synthetic prefix is unmapped. Upstream framework/loader maps are composed separately.
 */
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
    while (editIndex < mappings.length && mappings[editIndex].generated_end <= index) {
      const edit = mappings[editIndex++];
      delta = edit.original_end - edit.generated_end;
      expressionIndex = 0;
    }
    const edit = mappings[editIndex];
    const expressions = edit?.expressions ?? [];
    while (
      expressionIndex < expressions.length &&
      expressions[expressionIndex].generated_end <= index
    ) {
      expressionIndex++;
    }
    const expression = expressions[expressionIndex];
    const original =
      expression && expression.generated_start <= index
        ? expression.original_start + (expression.exact ? index - expression.generated_start : 0)
        : edit && edit.generated_start <= index
          ? edit.original_start
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
