import { mkdtempSync, writeFileSync, rmSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { TraceMap, originalPositionFor } from "@jridgewell/trace-mapping";
import { moduleSourceMap } from "@stillwind/host";
import { stillwind } from "@stillwind/integration/vite";
import { createServer } from "vite";
import { expect, test } from "vite-plus/test";

test("Vite development composes linked CSS maps through framework preprocessing", async () => {
  const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../../..");
  const root = mkdtempSync(path.join(repo, ".test-session-"));
  writeFileSync(path.join(root, "root.css"), "@utility red {color:red} @stillwind-slot utilities;");
  const source =
    "import {style} from 'stillwind/react';\n// 日本語 🧭\nexport const props=style('red', { '--x': progress },\n { 'data-value': data },\n { '--y': progress });";
  writeFileSync(path.join(root, "entry.ts"), source);
  const server = await createServer({
    configFile: false,
    root,
    resolve: {
      alias: { "stillwind/react/runtime": path.join(repo, "packages/core/src/runtime/react.ts") },
    },
    plugins: [
      {
        name: "framework-preprocessing-fixture",
        enforce: "pre",
        transform(code, id) {
          if (id !== path.join(root, "entry.ts").replaceAll("\\", "/")) {
            return;
          }
          const prefix = "// framework helper\n\n";
          return {
            code: prefix + code,
            map: JSON.stringify(moduleSourceMap(code, code, id, [], [], prefix)),
          };
        },
      },
      stillwind({
        css: path.join(root, "root.css"),
        types: false,
      }),
    ],
    css: {
      devSourcemap: true,
    },
    server: {
      middlewareMode: true,
      watch: null,
    },
    logLevel: "silent",
  });
  try {
    const result = await server.transformRequest("/entry.ts");
    expect(result).toBeTruthy();
    expect(result!.map).toBeTruthy();
    const jsMap = new TraceMap(JSON.stringify(result!.map));
    const generatedProgress = [...result!.code.matchAll(/\bprogress\b/g)];
    const originalProgress = [...source.matchAll(/\bprogress\b/g)];
    expect(generatedProgress.length, result!.code).toBe(2);
    for (let index = 0; index < generatedProgress.length; index++) {
      const generatedOffset = generatedProgress[index].index!;
      const originalOffset = originalProgress[index].index!;
      const origin = originalPositionFor(jsMap, {
        line: result!.code.slice(0, generatedOffset).split("\n").length,
        column: generatedOffset - result!.code.lastIndexOf("\n", generatedOffset - 1) - 1,
      });
      expect(origin.source?.endsWith("entry.ts"), JSON.stringify(origin)).toBeTruthy();
      expect(origin.line).toBe(source.slice(0, originalOffset).split("\n").length);
      expect(origin.column).toBe(originalOffset - source.lastIndexOf("\n", originalOffset - 1) - 1);
    }
    const css = await server.environments.client.pluginContainer.load("\0stillwind:snapshot.css");
    if (!(css && typeof css === "object")) {
      expect.fail('Unexpected value: css && typeof css === "object"');
    }
    expect(css.map, css.code).toBeTruthy();
    const map = new TraceMap(JSON.stringify(css.map));
    const offset = css.code.indexOf(".sw_");
    const prefix = css.code.slice(0, offset);
    const origin = originalPositionFor(map, {
      line: prefix.split("\n").length,
      column: offset - prefix.lastIndexOf("\n") - 1,
    });
    expect(origin.source?.endsWith("/entry.ts"), JSON.stringify(origin)).toBeTruthy();
    expect(origin.line).toBe(3);
    expect(origin.column).toBe("export const props=".length);
  } finally {
    await server.close();
    rmSync(root, { recursive: true, force: true });
  }
});

test("maps replacements and untouched UTF-16 positions after removed and inserted imports", () => {
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
          originalStart: start,
          originalEnd: start + 13,
          generatedStart: start,
          generatedEnd: start + replacement.length,
        },
      ],
      [{ from: 0, to: macro.length }],
      prefix,
    ),
  );
  expect(originalPositionFor(map, { line: 1, column: 0 }).source).toBe(null);
  expect(originalPositionFor(map, { line: 4, column: 20 })).toStrictEqual({
    source: "/src/a.tsx",
    line: 3,
    column: 10,
    name: null,
  });
  expect(originalPositionFor(map, { line: 5, column: 8 })).toStrictEqual({
    source: "/src/a.tsx",
    line: 4,
    column: 8,
    name: null,
  });
  expect(originalPositionFor(map, { line: 3, column: 18 })).toStrictEqual({
    source: "/src/a.tsx",
    line: 2,
    column: 18,
    name: null,
  });
  expect(map.sourcesContent?.[0]).toBe(source);
});
