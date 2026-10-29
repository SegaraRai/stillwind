import path from "node:path";
import { pathToFileURL } from "node:url";

import { GenMapping, addSegment, setSourceContent, toEncodedMap } from "@jridgewell/gen-mapping";
import { TraceMap, originalPositionFor } from "@jridgewell/trace-mapping";
import { expect, test } from "vite-plus/test";

import { cssWithSourceMap, decodeCssSourceMap } from "./css-source-map.ts";

test("multiline Unicode notices remain unmapped before the CSS body", () => {
  const result = decodeCssSourceMap(
    cssWithSourceMap(
      {
        css: ".a{}",
        banners: ["/*! 作者\n license */"],
        sourceMap: {
          sources: [
            {
              sourceMapSource: "input.css",
              content: ".a{}",
            },
          ],
          entries: [
            {
              source: 0,
              generatedStart: 0,
              generatedEnd: 4,
              originalStart: 0,
              originalEnd: 4,
              exact: true,
            },
          ],
        },
      },
      new Map(),
    ),
  );
  expect(result.code).toBe("/*! 作者\n license */\n.a{}");
  const trace = new TraceMap(result.map!);
  expect(originalPositionFor(trace, { line: 1, column: 4 }).source).toBeNull();
  expect(originalPositionFor(trace, { line: 3, column: 0 })).toMatchObject({
    source: "input.css",
    line: 1,
    column: 0,
  });
});

for (const suffix of ["?variant=foo/bar", "#variant=foo/bar"]) {
  test(`CSS producer maps resolve relative sources before the ${suffix} suffix`, () => {
    const id = path.resolve("map-project", "src", "input.ts");
    const sourceMapSource = pathToFileURL(id).href + suffix;
    const original = new GenMapping({ sourceRoot: "../authored/" });
    setSourceContent(original, "input.ts", "authored source");
    addSegment(original, 0, 0, "input.ts", 3, 2);
    const css = ".x{}";
    const result = decodeCssSourceMap(
      cssWithSourceMap(
        {
          css,
          sourceMap: {
            sources: [
              {
                sourceMapSource,
                content: "x",
              },
            ],
            entries: [
              {
                source: 0,
                generatedStart: 0,
                generatedEnd: css.length,
                originalStart: 0,
                originalEnd: 1,
                exact: false,
              },
            ],
          },
        },
        new Map(),
        {
          id: id + suffix,
          sourceMapSource,
          map: JSON.stringify(toEncodedMap(original)),
        },
      ),
    );
    expect(
      originalPositionFor(new TraceMap(result.map!), {
        line: 1,
        column: 0,
      }),
    ).toEqual({
      source: pathToFileURL(path.resolve("map-project", "authored", "input.ts")).href,
      line: 4,
      column: 2,
      name: null,
    });
    expect(result.map!.sourcesContent).toEqual(["authored source"]);
  });
}
