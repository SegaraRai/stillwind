import { createRequire } from "node:module";
import path from "node:path";

import { TraceMap, decodedMappings } from "@jridgewell/trace-mapping";

import type { PreparedSource } from "./protocol.ts";

/**
 * Checker input with optional independent original UTF-16 script region.
 */
export interface CheckerBuffer {
  source: string;
  language: "ts" | "tsx";
  region?: { start: number; end: number };
}

/**
 * Returns checker declarations selected by document.format for projected input.
 * Astro client scripts become independent buffers while the main buffer keeps their
 * line positions. The caller supplies projection maps and original script ranges;
 * source is not mutated. Plain input requires no extra declarations or buffers.
 * Throws if declaration packages cannot be resolved or Astro script mappings are invalid.
 */
export function checkerEnvironment(source: PreparedSource): {
  checkerBuffers?: CheckerBuffer[];
  checkerTypes: string[];
} {
  if (source.kind === "direct") {
    return { checkerTypes: [] };
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
    return { checkerTypes: [] };
  }
  let main = source.projection.code;
  const buffers: CheckerBuffer[] = [];
  for (const region of source.projection.checkerScriptRanges ?? []) {
    const text = source.document.code.slice(region.start, region.end);
    const at = clientProjectionStart(source, region);
    main = main.slice(0, at) + text.replace(/[^\r\n]/g, " ") + main.slice(at + text.length);
    buffers.push({ source: text, language: "ts", region });
  }
  return {
    checkerBuffers: [{ source: main, language: "tsx" }, ...buffers],
    checkerTypes: [path.join(path.dirname(userRequire.resolve("astro/package.json")), "env.d.ts")],
  };
}

/**
 * Locates a client body using original/generated UTF-16 map points and exact text.
 */
function clientProjectionStart(
  source: Extract<PreparedSource, { kind: "projected" }>,
  region: { start: number; end: number },
): number {
  if (source.projection.map.kind !== "v3") {
    throw new Error("Astro client scripts require a source map");
  }
  const trace = new TraceMap(source.projection.map.json);
  const selected = trace.sources.indexOf(source.projection.map.source);
  const lines = (text: string) => [0, ...[...text.matchAll(/\n/g)].map((match) => match.index + 1)];
  const generated = lines(source.projection.code),
    original = lines(source.document.code);
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
    throw new Error(`Cannot map Astro client script in ${source.moduleId}`);
  }
  return [...candidates][0];
}
