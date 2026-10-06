import { GenMapping, addSegment, setSourceContent, toEncodedMap } from "@jridgewell/gen-mapping";
import { cssWithSourceMap, decodeCssSourceMap } from "@stillwind/host";
import { expect, test } from "vite-plus/test";

import { cssWithSourceMap as candidate } from "./candidate.ts";
import { cssInput } from "./input.ts";

for (const exact of [true, false]) {
  test(`candidate retains Unicode, CRLF and replacement mappings (exact=${exact})`, () => {
    for (const rules of [1, 8, 64]) {
      const { output, replacements } = cssInput(rules, exact);
      for (const includeSourcesContent of [true, false]) {
        expect(
          decodeCssSourceMap(candidate(output, replacements, undefined, includeSourcesContent)),
        ).toStrictEqual(
          decodeCssSourceMap(
            cssWithSourceMap(output, replacements, undefined, includeSourcesContent),
          ),
        );
      }
    }
  });
}

test("candidate preserves multiple origins, mapping gaps and literal placeholders", () => {
  const css = "/* 🧭 */ .a{color:foo+bar}.b{color:foo}";
  const output = {
    css,
    sourceMap: {
      sources: [
        { sourceMapSource: "untitled:first.css", content: ".a{color:foo+bar}" },
        { sourceMapSource: "untitled:second.css", content: "// 日本語\nstyle('foo')" },
      ],
      entries: [
        {
          source: 1,
          generatedStart: Buffer.byteLength(css.slice(0, css.indexOf(".b"))),
          generatedEnd: Buffer.byteLength(css),
          originalStart: Buffer.byteLength("// 日本語\n"),
          originalEnd: Buffer.byteLength("// 日本語\nstyle('foo')"),
          exact: false,
        },
        {
          source: 0,
          generatedStart: Buffer.byteLength("/* 🧭 */ "),
          generatedEnd: Buffer.byteLength(css.slice(0, css.indexOf(".b"))),
          originalStart: 0,
          originalEnd: Buffer.byteLength(".a{color:foo+bar}"),
          exact: true,
        },
      ],
    },
  };
  const replacements = new Map([
    ["foo+bar", "x$&y"],
    ["foo", "42"],
  ]);
  expect(decodeCssSourceMap(candidate(output, replacements))).toStrictEqual(
    decodeCssSourceMap(cssWithSourceMap(output, replacements)),
  );
});

test("candidate composes an upstream map by its explicit source identity", () => {
  const { output, replacements } = cssInput(1);
  const map = new GenMapping();
  setSourceContent(map, "original.css", "/* original */\n" + output.css);
  addSegment(map, 0, 0, "original.css", 1, 0);
  const upstream = [
    {
      id: "/src/generated.css",
      sourceMapSource: output.sourceMap.sources[0].sourceMapSource,
      map: JSON.stringify(toEncodedMap(map)),
    },
  ];
  expect(decodeCssSourceMap(candidate(output, replacements, upstream))).toStrictEqual(
    decodeCssSourceMap(cssWithSourceMap(output, replacements, upstream)),
  );
});
