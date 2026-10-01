import path from "node:path";
import { pathToFileURL } from "node:url";

import { GenMapping, addSegment, setSourceContent, toEncodedMap } from "@jridgewell/gen-mapping";
import { TraceMap, originalPositionFor } from "@jridgewell/trace-mapping";
import { NativeDesignSystemLoader, NativeProjectSession } from "@stillwind/native";
import { expect, test } from "vite-plus/test";

import { cssWithSourceMap, decodeCssSourceMap } from "../src/css-source-map.ts";
import { composeModuleSourceMap, moduleSourceMap } from "../src/source-map.ts";

const location = (text: string, offset: number) => {
  const prefix = text.slice(0, offset);
  return { line: prefix.split("\n").length, column: offset - prefix.lastIndexOf("\n") - 1 };
};

test("CSS maps retain imported authored text, theme tokens and macro origins", () => {
  const loader = new NativeDesignSystemLoader();
  const root =
    "@import 'child.css';\n@theme static { --color-brand: red; } :root,:host { @stillwind-slot theme; }\n@utility red {color:red;}\n@stillwind-slot utilities;";
  const child = "/* 日本語 🧭 */\n.authored { color: blue; }";
  const designUri = pathToFileURL(path.resolve("design.css")).href;
  const childUri = pathToFileURL(path.resolve("child.css")).href;
  const step = loader.provide(0, "design.css", root, designUri, designUri);
  expect(step.kind).toBe("needFile");
  if (step.kind !== "needFile") {
    expect.fail("expected the imported CSS file to be requested");
  }
  loader.provide(step.requests[0].id, "child.css", child, childUri, childUri);
  const session = new NativeProjectSession(loader, false);
  const source =
    "import {style} from 'stillwind/react';\n// 日本語 🧭\nexport const props = style('red');";
  session.compile([
    {
      moduleId: "entry.tsx",
      kind: "direct",
      document: {
        uri: "untitled:entry.tsx",
        code: source,
        format: "jsx",
        sourceMapSource: pathToFileURL(path.resolve("entry.tsx")).href,
      },
      syntax: {
        language: "tsx",
        moduleKind: "unambiguous",
      },
    },
  ]);
  const output = session.planStylesheet({
    modules: ["entry.tsx"],
    coverage: { kind: "observed" },
    placement: "document",
  });
  const decoded = decodeCssSourceMap(cssWithSourceMap(output, new Map()));
  const projected = decodeCssSourceMap(cssWithSourceMap(output, new Map(), undefined, false));
  expect(projected.map.sourcesContent).toBe(undefined);
  expect(projected.code).toBe(decoded.code);
  expect(projected.map.sources).toStrictEqual(decoded.map.sources);
  expect(projected.map.mappings).toBe(decoded.map.mappings);
  const map = new TraceMap(decoded.map);
  for (const [needle, original, file, text] of [
    ["color: blue", child.indexOf("color: blue"), "child.css", child],
    ["--color-brand", root.indexOf("--color-brand"), "design.css", root],
    ["color:red", source.indexOf("style('red')"), "entry.tsx", source],
  ] as const) {
    expect(decoded.code, decoded.code).toContain(needle);
    const actual = originalPositionFor(map, location(decoded.code, decoded.code.indexOf(needle)));
    expect(actual).toStrictEqual({
      source: pathToFileURL(path.resolve(file)).href,
      ...location(text, original),
      name: null,
    });
  }
});

test("resource URL length edits preserve following non-ASCII CSS columns", () => {
  const css = "/* 🧭 */ .x { background:url(PLACEHOLDER); color:red; }";
  const output = {
    css,
    sourceMap: {
      sources: [{ sourceMapSource: "asset.css", content: css }],
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
    cssWithSourceMap(output, new Map([["PLACEHOLDER", "./much/longer/image.svg"]])),
  );
  const actual = originalPositionFor(
    new TraceMap(decoded.map),
    location(decoded.code, decoded.code.indexOf("color")),
  );
  expect(actual.column).toBe(css.indexOf("color"));
});

test("CSS origins compose through framework preprocessing to original source", () => {
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
      sources: [{ sourceMapSource: pathToFileURL(id).href, content: processed }],
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
    cssWithSourceMap(output, new Map(), {
      id,
      sourceMapSource: pathToFileURL(id).href,
      map: JSON.stringify(toEncodedMap(previous)),
    }),
  );
  const mapped = originalPositionFor(new TraceMap(result.map), { line: 1, column: 1 });
  expect(mapped).toStrictEqual({ source: pathToFileURL(id).href, line: 2, column: 17, name: null });
  expect(result.map.sourcesContent).toStrictEqual([original]);
});

test("loader composition traces native replacements through an upstream framework transform", () => {
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
        originalStart: start,
        originalEnd: input.length - 1,
        generatedStart: start,
        generatedEnd: compiled.length - 1,
      },
    ],
    [],
    "",
  );
  const composed = composeModuleSourceMap(generated, JSON.stringify(toEncodedMap(upstream)));
  const actual = originalPositionFor(new TraceMap(JSON.stringify(composed)), {
    line: 2,
    column: 25,
  });
  expect(actual.source).toBe(id);
  expect(actual.line).toBe(2);
  expect(actual.column).toBe(21);
  expect(composed.sourcesContent).toStrictEqual([original]);
});
