import { createRequire } from "node:module";
import path from "node:path";

import { TraceMap, decodedMappings } from "@jridgewell/trace-mapping";

import type { ProjectSource } from "./protocol.ts";

/** Checker input with optional independent original UTF-16 script region. */
export interface CheckerBuffer {
  source: string;
  language: "ts" | "tsx";
  region?: { start: number; end: number };
}

/** Supplies framework declarations and isolates client script scopes for checking. */
export function checkerEnvironment(source: ProjectSource): {
  checker_buffers?: CheckerBuffer[];
  checker_types: string[];
} {
  if (source.fw_code === undefined) {
    return { checker_types: [] };
  }
  const require = createRequire(import.meta.url);
  const userRequire = createRequire(path.resolve(source.id));
  if (source.framework === "Svelte") {
    return {
      checker_types: [
        require.resolve("svelte2tsx/svelte-shims-v4.d.ts"),
        require.resolve("svelte2tsx/svelte-jsx-v4.d.ts"),
      ],
    };
  }
  if (source.framework !== "Astro") {
    return { checker_types: [] };
  }
  let main = source.source;
  const buffers: CheckerBuffer[] = [];
  for (const region of source.checker_scripts ?? []) {
    const text = source.fw_code.slice(region.start, region.end);
    const at = clientProjectionStart(source, region);
    main = main.slice(0, at) + text.replace(/[^\r\n]/g, " ") + main.slice(at + text.length);
    buffers.push({ source: text, language: "ts", region });
  }
  return {
    checker_buffers: [{ source: main, language: "tsx" }, ...buffers],
    checker_types: [path.join(path.dirname(userRequire.resolve("astro/package.json")), "env.d.ts")],
  };
}

/** Locates a client body using original/generated UTF-16 map points and exact text. */
function clientProjectionStart(
  source: ProjectSource,
  region: { start: number; end: number },
): number {
  if (source.fw2js_map?.kind !== "v3") {
    throw new Error("Astro client scripts require a source map");
  }
  const trace = new TraceMap(source.fw2js_map.json);
  const selected = trace.sources.indexOf(source.fw2js_map.source);
  const lines = (text: string) => [0, ...[...text.matchAll(/\n/g)].map((match) => match.index + 1)];
  const generated = lines(source.source),
    original = lines(source.fw_code!);
  const text = source.fw_code!.slice(region.start, region.end);
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
      if (source.source.slice(start, start + text.length) === text) {
        candidates.add(start);
      }
    }
  }
  if (candidates.size !== 1) {
    throw new Error(`Cannot map Astro client script in ${source.path}`);
  }
  return [...candidates][0];
}
