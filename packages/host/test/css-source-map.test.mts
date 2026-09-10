import assert from "node:assert/strict";
import path from "node:path";
import test from "node:test";
import { pathToFileURL } from "node:url";

import { GenMapping, addSegment, setSourceContent, toEncodedMap } from "@jridgewell/gen-mapping";
import { TraceMap, originalPositionFor } from "@jridgewell/trace-mapping";
import { NativeDesignSystemLoader, NativeProjectSession } from "@stillwind/native";

import { cssWithSourceMap, decodeCssSourceMap } from "../src/css-source-map.ts";
import { composeModuleSourceMap, moduleSourceMap } from "../src/source-map.ts";

const location = (text: string, offset: number) => {
  const prefix = text.slice(0, offset);
  return { line: prefix.split("\n").length, column: offset - prefix.lastIndexOf("\n") - 1 };
};

void test("CSS maps retain imported authored text, theme tokens and macro origins", () => {
  const loader = new NativeDesignSystemLoader();
  const root =
    "@import 'child.css';\n@theme static { --color-brand: red; }\n@utility red {color:red;}\n@stillwind-slot utilities;";
  const child = "/* 日本語 🧭 */\n.authored { color: blue; }";
  const step = loader.provide(0, "design.css", root);
  loader.provide(step.requests[0].id, "child.css", child);
  const session = new NativeProjectSession(loader, false);
  const source =
    "import {style} from 'stillwind/react';\n// 日本語 🧭\nexport const props = style('red');";
  session.compile([{ path: "entry.tsx", source }]);
  const output = session.planStylesheet({
    modules: ["entry.tsx"],
    coverage: { kind: "observed" },
    placement: "document",
  });
  const decoded = decodeCssSourceMap(cssWithSourceMap(output, new Map(), process.cwd()));
  const projected = decodeCssSourceMap(
    cssWithSourceMap(output, new Map(), process.cwd(), undefined, false),
  );
  assert.equal(projected.map.sourcesContent, undefined);
  assert.equal(projected.code, decoded.code);
  assert.deepEqual(projected.map.sources, decoded.map.sources);
  assert.equal(projected.map.mappings, decoded.map.mappings);
  const map = new TraceMap(decoded.map);
  for (const [needle, original, file, text] of [
    ["color: blue", child.indexOf("color: blue"), "child.css", child],
    ["--color-brand", root.indexOf("--color-brand"), "design.css", root],
    ["color:red", source.indexOf("style('red')"), "entry.tsx", source],
  ] as const) {
    assert.ok(decoded.code.includes(needle), decoded.code);
    const actual = originalPositionFor(map, location(decoded.code, decoded.code.indexOf(needle)));
    assert.deepEqual(actual, {
      source: pathToFileURL(path.resolve(file)).href,
      ...location(text, original),
      name: null,
    });
  }
});

void test("resource URL length edits preserve following non-ASCII CSS columns", () => {
  const css = "/* 🧭 */ .x { background:url(PLACEHOLDER); color:red; }";
  const output = {
    css,
    sourceMap: {
      sources: [{ path: "asset.css", content: css }],
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
  const decoded = decodeCssSourceMap(
    cssWithSourceMap(output, new Map([["PLACEHOLDER", "./much/longer/image.svg"]]), process.cwd()),
  );
  const actual = originalPositionFor(
    new TraceMap(decoded.map),
    location(decoded.code, decoded.code.indexOf("color")),
  );
  assert.equal(actual.column, css.indexOf("color"));
});

void test("CSS origins compose through framework preprocessing to original source", () => {
  const id = path.resolve("route.tsx");
  const original = "// 日本語 🧭\nexport const x = style('red');";
  const processed = "// framework\n// helper\nexport const x = style('red');";
  const previous = new GenMapping({ file: id });
  setSourceContent(previous, id, original);
  addSegment(previous, 2, 17, id, 1, 17);
  const css = "._s_example{color:red}";
  const output = {
    css,
    sourceMap: {
      sources: [{ path: id, content: processed }],
      entries: [
        {
          source: 0,
          generatedStart: 0,
          generatedEnd: css.length,
          originalStart: Buffer.byteLength(processed.slice(0, processed.indexOf("style"))),
          originalEnd: Buffer.byteLength(processed),
          exact: false,
        },
      ],
    },
  };
  const result = decodeCssSourceMap(
    cssWithSourceMap(output, new Map(), process.cwd(), {
      id,
      map: JSON.stringify(toEncodedMap(previous)),
    }),
  );
  const mapped = originalPositionFor(new TraceMap(result.map), { line: 1, column: 1 });
  assert.deepEqual(mapped, { source: pathToFileURL(id).href, line: 2, column: 17, name: null });
  assert.deepEqual(result.map.sourcesContent, [original]);
});

void test("loader composition traces native replacements through an upstream framework transform", () => {
  const id = path.resolve("route.tsx");
  const original = "// original route\nexport const props = style('red');";
  const input = "// framework helper\nexport const props = style('red');";
  const upstream = new GenMapping({ file: id });
  setSourceContent(upstream, id, original);
  addSegment(upstream, 1, 21, id, 1, 21);
  const start = input.indexOf("style");
  const replacement = '{className:"x"}';
  const compiled = input.slice(0, start) + replacement + ";";
  const generated = moduleSourceMap(
    input,
    compiled,
    id,
    [
      {
        original_start: start,
        original_end: input.length - 1,
        generated_start: start,
        generated_end: compiled.length - 1,
      },
    ],
    [],
    "",
  );
  const composed = composeModuleSourceMap(generated, JSON.stringify(toEncodedMap(upstream)));
  const actual = originalPositionFor(new TraceMap(composed), { line: 2, column: 25 });
  assert.equal(actual.source, id);
  assert.equal(actual.line, 2);
  assert.equal(actual.column, 21);
  assert.deepEqual(composed.sourcesContent, [original]);
});
