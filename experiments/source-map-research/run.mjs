import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { writeFileSync, mkdirSync, readFileSync } from "node:fs";
import { performance } from "node:perf_hooks";
import { fileURLToPath } from "node:url";

import {
  cssWithSourceMap as baseline,
  decodeCssSourceMap,
} from "../../packages/host/src/css-source-map.ts";
import { finalizeReactModuleWithMap } from "../../packages/host/src/finalize.ts";
import { moduleSourceMap, composeModuleSourceMap } from "../../packages/host/src/source-map.ts";
import { cssWithSourceMap as candidate } from "./css-candidate.ts";
import {
  GenMapping,
  addSegment,
  setSourceContent,
  toEncodedMap,
  TraceMap,
  decodedMappings,
  originalPositionFor,
  generatedPositionFor,
} from "./deps.mjs";

const native = fileURLToPath(
  new URL(
    `./native/target/release/stillwind-sourcemap-research${process.platform === "win32" ? ".exe" : ""}`,
    import.meta.url,
  ),
);
const library =
  process.platform === "win32"
    ? "sourcemap_research_addon.dll"
    : process.platform === "darwin"
      ? "libsourcemap_research_addon.dylib"
      : "libsourcemap_research_addon.so";
const addonModule = { exports: {} };
process.dlopen(
  addonModule,
  fileURLToPath(new URL(`./native/target/release/${library}`, import.meta.url)),
);
const addon = addonModule.exports;
const root = process.cwd();
const median = (a) => [...a].sort((a, b) => a - b)[Math.floor(a.length / 2)];
function measure(fn, count = 5) {
  fn();
  const samples = [];
  for (let i = 0; i < count; i++) {
    const start = performance.now();
    fn();
    samples.push(performance.now() - start);
  }
  return median(samples);
}
function invoke(input, args = []) {
  const start = performance.now();
  const result = spawnSync(native, args, {
    input: JSON.stringify(input),
    encoding: "utf8",
    maxBuffer: 128 * 1024 * 1024,
  });
  assert.equal(result.status, 0, result.stderr);
  return { ...JSON.parse(result.stdout), processRoundtripMs: performance.now() - start };
}
function codecInput(map) {
  const trace = new TraceMap(map);
  return {
    file: map.file,
    sources: map.sources,
    contents: map.sourcesContent,
    tokens: decodedMappings(trace).flatMap((segments, line) =>
      segments.map((s) => [line, s[0], s.length > 1 ? s[1] : null, s[2] ?? 0, s[3] ?? 0]),
    ),
  };
}
function jsEncode(input) {
  const map = new GenMapping({ file: input.file });
  input.sources.forEach((s, i) => setSourceContent(map, s, input.contents[i]));
  for (const [line, col, source, ol, oc] of input.tokens) {
    if (source === null) addSegment(map, line, col);
    else addSegment(map, line, col, input.sources[source], ol, oc);
  }
  return JSON.stringify(toEncodedMap(map));
}
function compare(map, other) {
  // Encoded strings may differ; consumer-observable segments must be identical.
  assert.deepEqual(other.sources, map.sources);
  assert.equal(other.file, map.file);
  assert.deepEqual(other.sourcesContent, map.sourcesContent);
  assert.deepEqual(decodedMappings(new TraceMap(other)), decodedMappings(new TraceMap(map)));
}

// Deterministic generated valid inputs: exact spans, anchored generated rules,
// multiple sources, unmapped wrappers, Unicode, CRLF, empty and multiline URLs.
let seed = 0x12345678;
const random = (n) => {
  seed ^= seed << 13;
  seed ^= seed >>> 17;
  seed ^= seed << 5;
  return (seed >>> 0) % n;
};
const words = ["a", "é", "中", "🧭", "\r\n", "\n", "\t", "\\", '"', "\u2028", "\u2029"];
function fixture(count = 8) {
  let css = "/* wrapper */";
  const sources = [],
    entries = [],
    replacements = new Map();
  for (let i = 0; i < count; i++) {
    const prefix = Array.from({ length: random(12) }, () => words[random(words.length)]).join("");
    const key = `__stillwind_resource_${i.toString(16).padStart(64, "0")}__`;
    const exact = random(2) === 0;
    const text = `.c${i}{background:url(${key});content:"中🧭";}\r\n`;
    const original = exact ? text : `style('x${i}')`;
    sources.push({ path: `source${i}.tsx`, content: prefix + original + "\n// tail" });
    entries.push({
      source: i,
      generatedStart: Buffer.byteLength(css),
      generatedEnd: Buffer.byteLength(css + text),
      originalStart: Buffer.byteLength(prefix),
      originalEnd: Buffer.byteLength(prefix + original),
      exact,
    });
    css += text + "/* gap */";
    replacements.set(key, ["", "x", "日本🧭.svg", "a\r\nb"][random(4)]);
  }
  return { output: { css, sourceMap: { sources, entries } }, replacements };
}
let checks = 0;
// Exhaustive single replacements on a small Unicode/CRLF source, plus seeded
// multi-edit cases. Compare every segment, so both lookup biases are preserved.
const jobs = [],
  expected = [];
function jsCase(source, ranges, prefix = "") {
  let compiled = "",
    cursor = 0;
  const mappings = [];
  for (const [a, b, text] of ranges) {
    compiled += source.slice(cursor, a);
    const start = compiled.length;
    compiled += text;
    mappings.push({
      original_start: a,
      original_end: b,
      generated_start: start,
      generated_end: compiled.length,
    });
    cursor = b;
  }
  compiled += source.slice(cursor);
  jobs.push({
    source,
    id: "case.tsx",
    prefix,
    edits: ranges.map(([a, b, text]) => [
      Buffer.byteLength(source.slice(0, a)),
      Buffer.byteLength(source.slice(0, b)),
      text,
    ]),
  });
  expected.push({
    code: prefix + compiled,
    map: moduleSourceMap(source, compiled, "case.tsx", mappings, [], prefix),
  });
}
const small = "a🧭\r\né中\nxyz";
let unit = 0;
const boundaries = [0];
for (const char of small) {
  unit += char.length;
  boundaries.push(unit);
}
for (const a of boundaries)
  for (const b of boundaries)
    if (a <= b)
      for (const replacement of ["", "x", "🧭\r\n中"])
        jsCase(small, [[a, b, replacement]], "// p\n");
for (let i = 0; i < 200; i++) {
  let source = "",
    ranges = [];
  for (let j = 0; j < 10; j++) {
    source += words[random(words.length)];
    const a = source.length;
    source += words[random(words.length)];
    ranges.push([a, source.length, words[random(words.length)].repeat(random(3))]);
  }
  jsCase(source, ranges);
}
const batch = invoke(jobs, ["emit-batch"]);
batch.results.forEach((result, i) => {
  assert.equal(result.code, expected[i].code);
  compare(expected[i].map, result.map);
  checks++;
});
jobs.forEach((job, i) => {
  const result = JSON.parse(addon.emitSource(JSON.stringify(job)));
  assert.equal(result.code, expected[i].code);
  compare(expected[i].map, result.map);
  checks++;
});
for (let i = 0; i < 200; i++) {
  const { output, replacements } = fixture();
  assert.equal(candidate(output, replacements, root), baseline(output, replacements, root));
  checks++;
}
// Reordered and repeated fragments from the same source (original positions
// need not increase); original boundary checkpoints must not be global cursors.
for (let trial = 0; trial < 200; trial++) {
  const content = "a🧭\r\n中PLACEHOLDERé\nxyz";
  const bounds = [0];
  let pos = 0;
  for (const ch of content) {
    pos += ch.length;
    bounds.push(pos);
  }
  let css = "";
  const entries = [];
  for (let k = 0; k < 12; k++) {
    const a = random(bounds.length - 1),
      b = a + 1 + random(bounds.length - a - 1);
    const text = content.slice(bounds[a], bounds[b]);
    entries.push({
      source: 0,
      generatedStart: Buffer.byteLength(css),
      generatedEnd: Buffer.byteLength(css + text),
      originalStart: Buffer.byteLength(content.slice(0, bounds[a])),
      originalEnd: Buffer.byteLength(content.slice(0, bounds[b])),
      exact: true,
    });
    css += text;
  }
  const output = {
    css,
    sourceMap: { sources: [{ path: "shared.css", content }], entries: entries.reverse() },
  };
  const replacements = new Map([
    ["PLACEHOLDER", "long\r\n🧭"],
    ["xy", ""],
    ["a", "日本"],
  ]);
  assert.equal(candidate(output, replacements, root), baseline(output, replacements, root));
  checks++;
}
assert.equal(candidate({ css: "", sourceMap: { sources: [], entries: [] } }, new Map(), root), "");
checks++;
{
  const known = `__stillwind_resource_${"0".repeat(64)}__`;
  const unknown = `__stillwind_js_resource_${"1".repeat(64)}__`;
  const css = unknown + known + unknown;
  const output = {
    css,
    sourceMap: {
      sources: [{ path: "unknown.css", content: css }],
      entries: [
        {
          source: 0,
          generatedStart: 0,
          generatedEnd: css.length,
          originalStart: 0,
          originalEnd: css.length,
          exact: true,
        },
      ],
    },
  };
  assert.equal(
    candidate(output, new Map([[known, "x"]]), root),
    baseline(output, new Map([[known, "x"]]), root),
  );
  checks++;
}
// Generic literal keys: regex metacharacters, prefix priorities, no recursive replacement.
for (const pairs of [
  [
    ["a", "aa"],
    ["ab", "x"],
  ],
  [
    ["ab", "x"],
    ["a", "aa"],
  ],
  [
    [".*", "🧭"],
    ["$", "\n"],
  ],
  [["🧭", "中"]],
]) {
  const css = "ab.*$🧭ab";
  const output = {
    css,
    sourceMap: {
      sources: [{ path: "x.css", content: css }],
      entries: [
        {
          source: 0,
          generatedStart: 0,
          generatedEnd: Buffer.byteLength(css),
          originalStart: 0,
          originalEnd: Buffer.byteLength(css),
          exact: true,
        },
      ],
    },
  };
  assert.equal(candidate(output, new Map(pairs), root), baseline(output, new Map(pairs), root));
  checks++;
}
// Independent expected coordinates, not derived from the baseline.
const source = "/* 🧭 */\r\n.a{url(P);color:red}";
const simple = {
  css: source,
  sourceMap: {
    sources: [{ path: "x.css", content: source }],
    entries: [
      {
        source: 0,
        generatedStart: 0,
        generatedEnd: Buffer.byteLength(source),
        originalStart: 0,
        originalEnd: Buffer.byteLength(source),
        exact: true,
      },
    ],
  },
};
const decoded = decodeCssSourceMap(candidate(simple, new Map([["P", "long\n🧭"]]), root));
const index = decoded.code.indexOf("color");
const head = decoded.code.slice(0, index);
assert.deepEqual(
  originalPositionFor(new TraceMap(decoded.map), {
    line: head.split("\n").length,
    column: index - head.lastIndexOf("\n") - 1,
  }),
  { source: decoded.map.sources[0], line: 2, column: 10, name: null },
);
checks++;

const codecs = [];
for (const kind of ["js", "css"]) {
  let map;
  if (kind === "css") {
    const f = fixture(12);
    map = decodeCssSourceMap(candidate(f.output, f.replacements, root)).map;
  } else {
    const text = '// 🧭\r\nconst x = style("red");\n';
    const start = text.indexOf("style");
    const replacement = '({\nclassName:"x🧭"\n})';
    const end = text.indexOf(";");
    map = moduleSourceMap(
      text,
      text.slice(0, start) + replacement + text.slice(end),
      "input.tsx",
      [
        {
          original_start: start,
          original_end: end,
          generated_start: start,
          generated_end: start + replacement.length,
        },
      ],
      [],
      "// synthetic\n",
    );
  }
  const input = codecInput(map);
  const result = invoke(input);
  compare(map, result.map);
  const trace = new TraceMap(result.map);
  // Every mapped segment must also remain reachable through reverse lookup.
  for (const [line, col, src, ol, oc] of input.tokens)
    if (src !== null) {
      assert.equal(originalPositionFor(trace, { line: line + 1, column: col }).line, ol + 1);
      assert.ok(
        generatedPositionFor(trace, { source: input.sources[src], line: ol + 1, column: oc })
          .line !== null,
      );
    }
  compare(map, JSON.parse(addon.encodeTokens(JSON.stringify(input))));
  codecs.push({
    kind,
    tokens: input.tokens.length,
    oxcMs: result.encodeMs,
    jsMs: measure(() => jsEncode(input)),
    napiIncludingJsonMs: measure(() => JSON.parse(addon.encodeTokens(JSON.stringify(input)))),
    processRoundtripMs: result.processRoundtripMs,
  });
  checks++;
}
// Preserve upstream composition, including source content and sourceRoot resolution.
const f = fixture(1);
const d = decodeCssSourceMap(candidate(f.output, f.replacements, root));
const upstream = new GenMapping();
setSourceContent(upstream, "original.tsx", "original");
addSegment(upstream, 0, 0, "original.tsx", 0, 0);
const nativeMap = invoke(codecInput(d.map)).map;
assert.deepEqual(
  composeModuleSourceMap(nativeMap, toEncodedMap(upstream)),
  composeModuleSourceMap(d.map, toEncodedMap(upstream)),
);
checks++;

const cssBench = [];
for (const resources of [1, 64, 256]) {
  const f = fixture(resources);
  // Keep output size approximately constant while varying number of candidates.
  const text = "/* padding */\n".repeat(12000);
  f.output.css += text;
  assert.equal(candidate(f.output, f.replacements, root), baseline(f.output, f.replacements, root));
  cssBench.push({
    resources,
    bytes: Buffer.byteLength(f.output.css),
    baselineMs: measure(() => baseline(f.output, f.replacements, root)),
    candidateMs: measure(() => candidate(f.output, f.replacements, root)),
  });
}
const exactCss = '.x{color:red;content:"中🧭"}\r\n'.repeat(6000);
const exactOutput = {
  css: exactCss,
  sourceMap: {
    sources: [{ path: "dense.css", content: exactCss }],
    entries: [
      {
        source: 0,
        generatedStart: 0,
        generatedEnd: Buffer.byteLength(exactCss),
        originalStart: 0,
        originalEnd: Buffer.byteLength(exactCss),
        exact: true,
      },
    ],
  },
};
assert.equal(candidate(exactOutput, new Map(), root), baseline(exactOutput, new Map(), root));
checks++;
cssBench.push({
  kind: "dense-exact-no-urls",
  resources: 0,
  bytes: Buffer.byteLength(exactCss),
  baselineMs: measure(() => baseline(exactOutput, new Map(), root)),
  candidateMs: measure(() => candidate(exactOutput, new Map(), root)),
});
const large = fixture(64);
const largeMap = decodeCssSourceMap(candidate(large.output, large.replacements, root)).map;
const denseSource = 'const example = "🧭中";\n'.repeat(10000);
const dense = moduleSourceMap(denseSource, denseSource, "dense.tsx", [], [], "");
for (const [kind, map] of [
  ["css-large", largeMap],
  ["js-dense", dense],
]) {
  const input = codecInput(map);
  const result = invoke(input);
  compare(map, result.map);
  codecs.push({
    kind,
    tokens: input.tokens.length,
    oxcMs: result.encodeMs,
    jsMs: measure(() => jsEncode(input)),
    napiIncludingJsonMs: measure(() => JSON.parse(addon.encodeTokens(JSON.stringify(input)))),
    processRoundtripMs: result.processRoundtripMs,
  });
}
const nativeEmission = [];
for (const count of [0, 1, 25, 1000]) {
  let source = "",
    compiled = "",
    mappings = [],
    edits = [];
  for (let i = 0; i < count; i++) {
    const prefix = "// 日本語 🧭\r\nconst x = ";
    source += prefix;
    compiled += prefix;
    const original_start = source.length,
      byteStart = Buffer.byteLength(source);
    source += 'style("red")';
    const replacement = ["", '({\nclassName:"🧭"\n})', '"x"'][i % 3];
    edits.push([byteStart, Buffer.byteLength(source), replacement]);
    const generated_start = compiled.length;
    compiled += replacement;
    mappings.push({
      original_start,
      original_end: source.length,
      generated_start,
      generated_end: compiled.length,
    });
    source += ";\n";
    compiled += ";\n";
  }
  source += "// end\n";
  compiled += "// end\n";
  const prefix = "// synthetic 🧭\r\n";
  const map = moduleSourceMap(source, compiled, "emit.tsx", mappings, [], prefix);
  const result = invoke({ source, id: "emit.tsx", prefix, edits }, ["emit"]);
  assert.equal(result.code, prefix + compiled);
  compare(map, result.map);
  checks++;
  nativeEmission.push({
    edits: count,
    bytes: Buffer.byteLength(source),
    rustEmitMs: result.emitMs,
    napiIncludingJsonMs: measure(() =>
      JSON.parse(addon.emitSource(JSON.stringify({ source, id: "emit.tsx", prefix, edits }))),
    ),
    hostMapOnlyMs: measure(() =>
      JSON.stringify(moduleSourceMap(source, compiled, "emit.tsx", mappings, [], prefix)),
    ),
    processRoundtripMs: result.processRoundtripMs,
  });
}
// The actual host finalizer remains a second edit stage. Test composition rather
// than assuming a faster native encoder makes the complete pipeline faster.
const finalSource =
  'import { style } from "stillwind/react";\r\n// 🧭\nexport const x = style("red");\n';
const finalStart = finalSource.indexOf('style("');
const finalEnd = finalSource.indexOf(";", finalStart);
const finalReplacement = 'stillwindClsx({\nclassName:"x"\n})';
const finalCompiled =
  finalSource.slice(0, finalStart) + finalReplacement + finalSource.slice(finalEnd);
const finalEdits = [
  {
    original_start: finalStart,
    original_end: finalEnd,
    generated_start: finalStart,
    generated_end: finalStart + finalReplacement.length,
  },
];
const finalized = finalizeReactModuleWithMap(finalCompiled, "./carrier.css");
const direct = moduleSourceMap(
  finalSource,
  finalCompiled,
  "final.tsx",
  finalEdits,
  finalized.removed,
  finalized.prefix,
);
const first = JSON.parse(
  addon.emitSource(
    JSON.stringify({
      source: finalSource,
      id: "final.tsx",
      prefix: "",
      edits: [
        [
          Buffer.byteLength(finalSource.slice(0, finalStart)),
          Buffer.byteLength(finalSource.slice(0, finalEnd)),
          finalReplacement,
        ],
      ],
    }),
  ),
).map;
const composed = composeModuleSourceMap(
  moduleSourceMap(
    finalCompiled,
    finalCompiled,
    "final.tsx",
    [],
    finalized.removed,
    finalized.prefix,
  ),
  first,
);
let glbDifferences = 0,
  lubDifferences = 0,
  line = 1,
  column = 0;
for (const unit of finalized.code.split("")) {
  for (const bias of [1, -1]) {
    const a = originalPositionFor(new TraceMap(direct), { line, column, bias });
    const b = originalPositionFor(new TraceMap(composed), { line, column, bias });
    if (JSON.stringify(a) !== JSON.stringify(b)) {
      if (bias === 1) glbDifferences++;
      else lubDifferences++;
    }
  }
  if (unit === "\n") {
    line++;
    column = 0;
  } else column++;
}
assert.equal(glbDifferences, 0);
checks++;
const finalization = {
  glbDifferences,
  lubDifferences,
  directSegments: codecInput(direct).tokens.length,
  composedSegments: codecInput(composed).tokens.length,
};
const memory = [];
for (const name of ["baseline", "candidate"]) {
  const result = spawnSync(
    process.execPath,
    ["--expose-gc", fileURLToPath(new URL("./memory.mjs", import.meta.url)), name],
    { encoding: "utf8" },
  );
  assert.equal(result.status, 0, result.stderr);
  memory.push(JSON.parse(result.stdout));
}
const editProcess = spawnSync(native, ["bench-edits"], { encoding: "utf8" });
assert.equal(editProcess.status, 0, editProcess.stderr);
const baselineFiles = [
  "crates/compiler/src/transforms.rs",
  "crates/napi/src/api.rs",
  "crates/source/src/text.rs",
  "packages/host/src/css-source-map.ts",
  "packages/host/src/source-map.ts",
  "packages/host/src/finalize.ts",
];
const baselineHashes = Object.fromEntries(
  baselineFiles.map((file) => [
    file,
    createHash("sha256").update(readFileSync(file)).digest("hex"),
  ]),
);
const revision = spawnSync("git", ["rev-parse", "HEAD"], { encoding: "utf8" }).stdout.trim();
const results = {
  date: new Date().toISOString(),
  node: process.version,
  revision,
  baselineHashes,
  checks,
  cssBench,
  codecs,
  nativeEmission,
  finalization,
  memory,
  rustEdits: JSON.parse(editProcess.stdout),
};
mkdirSync(new URL("./results/", import.meta.url), { recursive: true });
writeFileSync(
  new URL("./results/timings.json", import.meta.url),
  JSON.stringify(results, null, 2) + "\n",
);
console.log(JSON.stringify(results, null, 2));
