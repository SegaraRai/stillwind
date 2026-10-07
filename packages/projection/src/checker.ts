import { createRequire } from "node:module";
import path from "node:path";

import { TraceMap, decodedMappings } from "@jridgewell/trace-mapping";

import { PreparationError } from "./preparation.ts";
import type { PreparedSource } from "./protocol.ts";

/** Input text for one independent TypeScript checker file before Stillwind applies checker edits. */
export interface CheckerBuffer {
  /** Complete script text or the exact original script body identified by `region`. */
  source: string;
  /** Syntax used to parse the buffer. JSX expressions require `tsx`. */
  language: "ts" | "tsx";
  /** Original script bounds when buffer positions map directly into one independent script body. */
  region?: {
    /** Inclusive UTF-16 offset in the original document corresponding to buffer offset zero. */
    start: number;
    /** Exclusive UTF-16 offset in the original document, equal to `start + source.length`. */
    end: number;
  };
}

/** Additional declarations and optional independent code buffers needed to check a prepared source. */
export interface CheckerEnvironment {
  /**
   * Buffers that replace the default checker input. Astro places the main TSX buffer first, then
   * client scripts in source order. Omission uses the prepared document or projection code.
   */
  checkerBuffers?: CheckerBuffer[];
  /** Absolute paths of declarations required by the source format. Direct sources use an empty list. */
  checkerTypes: string[];
}

/**
 * Selects checker declarations and separates Astro client scripts from the main TSX buffer.
 *
 * Client script text is replaced with spaces in the main buffer while line breaks are preserved.
 * The prepared source is not mutated.
 *
 * @param source - Prepared source with the original text and any projection map and script ranges.
 * @returns Required declaration paths and any independent checker buffers. Direct and Vue sources
 * need no additional declarations or buffers from this function.
 * @throws If declaration packages cannot be resolved or an Astro script cannot be uniquely located
 * in its projection.
 */
export function checkerEnvironment(source: PreparedSource): CheckerEnvironment {
  if (source.kind === "direct") {
    return {
      checkerTypes: [],
    };
  }
  const require = createRequire(import.meta.url);
  const userRequire = createRequire(path.resolve(source.checkerFileName));
  if (source.document.format === "svelte") {
    return {
      checkerTypes: [
        require.resolve("svelte2tsx/svelte-shims-v4.d.ts"),
        require.resolve("svelte2tsx/svelte-jsx-v4.d.ts"),
      ],
    };
  }
  if (source.document.format !== "astro") {
    return {
      checkerTypes: [],
    };
  }
  let main = source.projection.code;
  const buffers: CheckerBuffer[] = [];
  for (const region of source.projection.checkerScriptRanges ?? []) {
    const text = source.document.code.slice(region.start, region.end);
    const at = clientProjectionStart(source, region);
    main = main.slice(0, at) + text.replace(/[^\r\n]/g, " ") + main.slice(at + text.length);
    buffers.push({
      source: text,
      language: "ts",
      region,
    });
  }
  return {
    checkerBuffers: [
      {
        source: main,
        language: "tsx",
      },
      ...buffers,
    ],
    checkerTypes: [path.join(path.dirname(userRequire.resolve("astro/package.json")), "env.d.ts")],
  };
}

/**
 * Locates an unchanged Astro client script body in generated code using its source-map points.
 *
 * @param source - Projected Astro source whose version 3 map identifies the original document.
 * @param region - Nonempty original script body with half-open UTF-16 bounds.
 * @returns The unique inclusive UTF-16 offset at which the complete original body occurs in the
 * projection and agrees with its mapped points.
 * @throws {PreparationError} If the map is not version 3 or no unique matching location exists.
 * @throws If the source map cannot be decoded.
 */
function clientProjectionStart(
  source: Extract<PreparedSource, { kind: "projected" }>,
  region: { start: number; end: number },
): number {
  if (source.projection.map.kind !== "v3") {
    throw new PreparationError(
      {
        kind: "projection",
        detail: "Astro client scripts require a source map",
      },
      source.projection.converter,
    );
  }

  const trace = new TraceMap(source.projection.map.json);
  const selected = trace.sources.indexOf(source.projection.map.source);
  const lines = (text: string) => [0, ...[...text.matchAll(/\n/g)].map((match) => match.index + 1)];
  const generated = lines(source.projection.code);
  const original = lines(source.document.code);
  const text = source.document.code.slice(region.start, region.end);
  const candidates = new Set<number>();
  for (const [line, segments] of decodedMappings(trace).entries()) {
    for (const segment of segments) {
      if (segment.length < 4 || segment[1] !== selected) {
        continue;
      }
      const offset = original[segment[2]!] + segment[3]!;
      if (offset < region.start || offset >= region.end) {
        continue;
      }
      const start = generated[line] + segment[0] - (offset - region.start);
      if (source.projection.code.slice(start, start + text.length) === text) {
        candidates.add(start);
      }
    }
  }
  if (candidates.size !== 1) {
    throw new PreparationError(
      {
        kind: "projection",
        detail: "Cannot uniquely map the Astro client script into its checker buffer",
      },
      source.projection.converter,
    );
  }

  return [...candidates][0];
}
