import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync, rmSync } from "node:fs";
import path from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { TraceMap, originalPositionFor } from "@jridgewell/trace-mapping";
import { moduleSourceMap } from "@stillwind/host";
import { createServer } from "vite-plus";

import { stillwind } from "../src/index.ts";

void test("Vite development composes carrier CSS maps through framework preprocessing", async () => {
  const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../..");
  const root = mkdtempSync(path.join(repo, ".session-test-"));
  writeFileSync(path.join(root, "root.css"), "@utility red {color:red} @stillwind-slot utilities;");
  const source =
    "import {style} from 'stillwind/react';\n// 日本語 🧭\nexport const props=style('red');";
  writeFileSync(path.join(root, "entry.ts"), source);
  const server = await createServer({
    configFile: false,
    root,
    plugins: [
      {
        name: "framework-preprocessing-fixture",
        enforce: "pre",
        transform(code, id) {
          if (id !== path.join(root, "entry.ts").replaceAll("\\", "/")) return;
          const prefix = "// framework helper\n\n";
          return {
            code: prefix + code,
            map: JSON.stringify(moduleSourceMap(code, code, id, [], [], prefix)),
          };
        },
      },
      stillwind({ css: path.join(root, "root.css"), types: false }),
    ],
    css: { devSourcemap: true },
    server: { middlewareMode: true, watch: null },
    logLevel: "silent",
  });
  try {
    const result = await server.transformRequest("/entry.ts");
    assert.ok(result);
    const carrier = result.code.match(/(?:from |import )"([^"\n]+stillwind-carrier\.css[^"\n]+)"/);
    assert.ok(carrier, result.code);
    const css = await server.transformRequest(carrier[1] + "&direct");
    assert.ok(css);
    assert.ok(css.map, css.code);
    const map = new TraceMap(JSON.stringify(css.map));
    const offset = css.code.indexOf("._s_");
    const prefix = css.code.slice(0, offset);
    const origin = originalPositionFor(map, {
      line: prefix.split("\n").length,
      column: offset - prefix.lastIndexOf("\n") - 1,
    });
    assert.ok(origin.source?.endsWith("/entry.ts"), JSON.stringify(origin));
    assert.equal(origin.line, 3);
    assert.equal(origin.column, "export const props=".length);
  } finally {
    await server.close();
    rmSync(root, { recursive: true, force: true });
  }
});

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
