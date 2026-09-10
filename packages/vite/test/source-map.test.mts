import assert from "node:assert/strict";
import { test } from "node:test";

import { TraceMap, originalPositionFor } from "@jridgewell/trace-mapping";
import { moduleSourceMap } from "@stillwind/host";

void test("maps replacements and untouched UTF-16 positions after removed and inserted imports", () => {
  const macro = 'import { style } from "stillwind/react";\r\n';
  const source = macro + 'const emoji = "😀";\r\nconst a = style("flex");\r\nconsole.log(a);';
  const start = source.indexOf('style("flex")');
  const replacement = '{ className: "_s_example" }';
  const compiled = source.slice(0, start) + replacement + source.slice(start + 13);
  const prefix = 'import "base.css";\nimport "module.css";\n';
  const map = new TraceMap(
    moduleSourceMap(
      source,
      compiled,
      "/src/a.tsx",
      [
        {
          original_start: start,
          original_end: start + 13,
          generated_start: start,
          generated_end: start + replacement.length,
        },
      ],
      [{ from: 0, to: macro.length }],
      prefix,
    ),
  );
  assert.equal(originalPositionFor(map, { line: 1, column: 0 }).source, null);
  assert.deepEqual(originalPositionFor(map, { line: 4, column: 20 }), {
    source: "/src/a.tsx",
    line: 3,
    column: 10,
    name: null,
  });
  assert.deepEqual(originalPositionFor(map, { line: 5, column: 8 }), {
    source: "/src/a.tsx",
    line: 4,
    column: 8,
    name: null,
  });
  assert.deepEqual(originalPositionFor(map, { line: 3, column: 18 }), {
    source: "/src/a.tsx",
    line: 2,
    column: 18,
    name: null,
  });
  assert.equal(map.sourcesContent?.[0], source);
});
