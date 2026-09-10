import { mkdirSync, writeFileSync, readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { performance } from "node:perf_hooks";

import { TraceMap, originalPositionFor, decodedMappings } from "@jridgewell/trace-mapping";
import { SourceMap as VolarMap } from "@volar/source-map";
import { parseSync } from "oxc-parser";

import { adapters } from "./adapters.mjs";
import { fixtures } from "./fixtures.mjs";

const require = createRequire(import.meta.url);
function starts(text) {
  const a = [0];
  for (let i = 0; i < text.length; i++) if (text[i] === "\n") a.push(i + 1);
  return a;
}
function location(lines, offset) {
  let lo = 0,
    hi = lines.length;
  while (lo + 1 < hi) {
    const mid = (lo + hi) >> 1;
    if (lines[mid] <= offset) lo = mid;
    else hi = mid;
  }
  return { line: lo + 1, column: offset - lines[lo] };
}
function nodesOf(node, result = []) {
  if (!node || typeof node !== "object") return result;
  if (typeof node.type === "string") result.push(node);
  for (const [k, v] of Object.entries(node)) {
    if (k === "parent") continue;
    if (Array.isArray(v)) v.forEach((x) => nodesOf(x, result));
    else if (v && typeof v === "object") nodesOf(v, result);
  }
  return result;
}
function hasMarker(node, marker) {
  return nodesOf(node, []).some((n) => n.type === "Literal" && n.value === marker);
}
function mapping(result, source) {
  if (result.mappings) {
    const map = new VolarMap(result.mappings);
    return {
      kind: "volar",
      point: (offset) => [...map.toSourceLocation(offset)].map(([p]) => p),
      range: (a, b) => [...map.toSourceRange(a, b, false)].map(([s, e]) => [s, e]),
    };
  }
  const raw =
    typeof result.map === "string"
      ? JSON.parse(result.map)
      : JSON.parse(JSON.stringify(result.map));
  const trace = new TraceMap(raw),
    gl = starts(result.code),
    ol = starts(source);
  return {
    kind: "v3",
    raw,
    segments: decodedMappings(trace).reduce((n, x) => n + x.length, 0),
    point(offset) {
      const p = originalPositionFor(trace, location(gl, offset));
      return p.source !== null && p.line !== null && ol[p.line - 1] !== undefined
        ? [ol[p.line - 1] + p.column]
        : [];
    },
  };
}
const rows = [];
mkdirSync("results/outputs", { recursive: true });
mkdirSync("fixtures", { recursive: true });
for (const fixture of fixtures)
  writeFileSync(`fixtures/${fixture.language}-${fixture.name}.${fixture.language}`, fixture.source);
for (const adapter of adapters) {
  for (const fixture of fixtures.filter((f) => f.language === adapter.language)) {
    const filename = `${fixture.language}-${fixture.name}.${fixture.language}`;
    const row = { adapter: adapter.name, fixture: fixture.name, language: fixture.language };
    try {
      const start = performance.now();
      const result = await adapter.convert(fixture.source, filename);
      row.elapsedMs = performance.now() - start;
      const parsed = parseSync(filename + result.extension, result.code);
      row.parseErrors = parsed.errors.map((e) => e.message);
      const nodes = nodesOf(parsed.program),
        mapper = mapping(result, fixture.source);
      row.mapKind = mapper.kind;
      row.generatedLength = result.code.length;
      row.segments = mapper.segments;
      writeFileSync(
        `results/outputs/${adapter.name}-${fixture.name}${result.extension}`,
        result.code,
      );
      writeFileSync(
        `results/outputs/${adapter.name}-${fixture.name}.map.json`,
        JSON.stringify(mapper.raw ?? result.mappings, null, 2),
      );
      row.probes = fixture.probes.map((probe) => {
        const candidates = nodes.filter((node) =>
          probe.kind === "import"
            ? node.type === "ImportDeclaration" && node.source.value === probe.module
            : node.type === "CallExpression" &&
              node.arguments.some((arg) => hasMarker(arg, probe.marker)),
        );
        const examined = candidates.map((node) => {
          const text = result.code.slice(node.start, node.end);
          const mappedStarts = mapper.point(node.start),
            mappedEnds = mapper.point(node.end),
            mappedLast = mapper.point(node.end - 1);
          const rawEqual = text === probe.raw;
          const correctStart = mappedStarts.includes(probe.start);
          const correctExclusiveEnd = mappedEnds.includes(probe.end);
          const correctLastEnd = mappedLast.includes(probe.end - 1);
          const startAndContent =
            correctStart &&
            rawEqual &&
            fixture.source.slice(probe.start, probe.start + text.length) === text;
          let scalarPoints = 0,
            exactScalarPoints = 0;
          if (rawEqual)
            for (let offset = 0; offset < text.length;) {
              scalarPoints++;
              if (mapper.point(node.start + offset).includes(probe.start + offset))
                exactScalarPoints++;
              offset += text.codePointAt(offset) > 0xffff ? 2 : 1;
            }
          const directBinding = probe.kind === "import" || node.callee.type === "Identifier";
          const nativeRange = mapper.range?.(node.start, node.end) ?? [];
          return {
            generatedStart: node.start,
            generatedEnd: node.end,
            text,
            mappedStarts,
            mappedEnds,
            rawEqual,
            correctStart,
            correctExclusiveEnd,
            correctLastEnd,
            startAndContent,
            directBinding,
            scalarPoints,
            exactScalarPoints,
            nativeRange,
          };
        });
        examined.sort(
          (a, b) =>
            Number(b.startAndContent) * 10 +
            Number(b.correctStart) * 3 +
            Number(b.rawEqual) -
            (Number(a.startAndContent) * 10 + Number(a.correctStart) * 3 + Number(a.rawEqual)),
        );
        return { ...probe, candidates: examined };
      });
    } catch (e) {
      row.error = e.message;
    }
    rows.push(row);
  }
}
const summary = adapters.map((adapter) => {
  const own = rows.filter((r) => r.adapter === adapter.name),
    probes = own.flatMap((r) => r.probes ?? []);
  const count = (fn) => probes.filter(fn).length;
  const region = (kind) =>
    probes.filter((p) => (p.kind === "import" ? "import" : p.region) === kind);
  return {
    adapter: adapter.name,
    fixtures: own.length,
    conversionErrors: own.filter((r) => r.error).length,
    parseErrorFiles: own.filter((r) => r.parseErrors?.length).length,
    probes: probes.length,
    rawPreserved: count((p) => p.candidates.some((c) => c.rawEqual)),
    startAndContent: count((p) => p.candidates.some((c) => c.startAndContent)),
    directAndContent: count((p) => p.candidates.some((c) => c.startAndContent && c.directBinding)),
    exclusiveEnd: count((p) =>
      p.candidates.some((c) => c.startAndContent && c.correctExclusiveEnd),
    ),
    lastEnd: count((p) => p.candidates.some((c) => c.startAndContent && c.correctLastEnd)),
    fullyMapped: count((p) =>
      p.candidates.some((c) => c.startAndContent && c.exactScalarPoints === c.scalarPoints),
    ),
    byRegion: Object.fromEntries(
      ["import", "script", "template", "module-script", "client-script", "normal-script"].map(
        (kind) => [
          kind,
          {
            total: region(kind).length,
            accepted: region(kind).filter((p) =>
              p.candidates.some((c) => c.startAndContent && c.directBinding),
            ).length,
          },
        ],
      ),
    ),
  };
});
const packages = [
  "@astrojs/compiler",
  "@astrojs/compiler-rs",
  "svelte",
  "svelte2tsx",
  "@vue/compiler-sfc",
  "@vue/language-core",
  "@volar/source-map",
  "oxc-parser",
  "typescript",
];
const versions = Object.fromEntries(
  packages.map((p) => {
    try {
      return [p, JSON.parse(readFileSync(require.resolve(p + "/package.json"), "utf8")).version];
    } catch {
      return [p, JSON.parse(readFileSync("package.json", "utf8")).devDependencies[p]];
    }
  }),
);
writeFileSync(
  "results/research.json",
  JSON.stringify(
    { date: new Date().toISOString(), node: process.version, versions, summary, rows },
    null,
    2,
  ) + "\n",
);
console.table(summary.map(({ byRegion, ...row }) => row));
for (const row of rows)
  if (row.error || row.parseErrors?.length)
    console.log(row.adapter, row.fixture, row.error ?? row.parseErrors);
