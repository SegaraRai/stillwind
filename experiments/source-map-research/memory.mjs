import { performance } from "node:perf_hooks";

import { cssWithSourceMap as baseline } from "../../packages/host/src/css-source-map.ts";
import { cssWithSourceMap as candidate } from "./css-candidate.ts";
const fn = process.argv[2] === "baseline" ? baseline : candidate;
const sources = [],
  entries = [];
let css = "";
for (let i = 0; i < 4; i++) {
  const content = "/* 日本語 🧭 */\n".repeat(25000) + `style('red')`;
  sources.push({ path: `source${i}.tsx`, content });
  const text = `.x${i}{color:red}\n`;
  entries.push({
    source: i,
    generatedStart: Buffer.byteLength(css),
    generatedEnd: Buffer.byteLength(css + text),
    originalStart: Buffer.byteLength(content) - 12,
    originalEnd: Buffer.byteLength(content),
    exact: false,
  });
  css += text;
}
global.gc();
const start = performance.now();
const output = fn(
  { css, sourceMap: { sources, entries } },
  new Map(),
  process.cwd(),
  undefined,
  false,
);
console.log(
  JSON.stringify({
    implementation: process.argv[2],
    elapsedMs: performance.now() - start,
    peakRssKiB: process.resourceUsage().maxRSS,
    outputBytes: Buffer.byteLength(output),
    sourceBytes: sources.reduce((n, s) => n + Buffer.byteLength(s.content), 0),
  }),
);
