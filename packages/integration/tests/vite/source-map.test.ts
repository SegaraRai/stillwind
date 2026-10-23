import { mkdtempSync, writeFileSync, rmSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { TraceMap, originalPositionFor } from "@jridgewell/trace-mapping";
import { moduleSourceMap } from "@stillwind/host";
import { stillwind } from "@stillwind/integration/vite";
import vue from "@vitejs/plugin-vue";
import { build, createServer } from "vite";
import { describe, expect, onTestFinished, test } from "vite-plus/test";

test("JavaScript arguments map to their original positions after a framework adds lines", async () => {
  const { server, source } = await createPreprocessedProject();

  const transformed = await server.transformRequest("/entry.ts");
  if (!transformed?.map) {
    expect.fail("Expected transformed JavaScript with a source map");
  }
  const map = new TraceMap(JSON.stringify(transformed.map));
  const generatedArguments = [...transformed.code.matchAll(/\bprogress\b/g)];
  const originalArguments = [...source.matchAll(/\bprogress\b/g)];

  expect(generatedArguments).toHaveLength(2);
  for (const [index, argument] of generatedArguments.entries()) {
    const original = originalPositionFor(map, positionAt(transformed.code, argument.index));
    expect(original).toMatchObject({
      source: expect.stringMatching(/entry\.ts$/),
      ...positionAt(source, originalArguments[index].index),
    });
  }
});

test("CSS classes map to the original style call after a framework adds lines", async () => {
  const { server, source } = await createPreprocessedProject();
  await server.transformRequest("/entry.ts");

  const cssModule = await server.environments.client.pluginContainer.load(
    "\0stillwind:snapshot.css",
  );
  if (!cssModule || typeof cssModule !== "object" || !cssModule.map) {
    expect.fail("Expected the CSS snapshot module with a source map");
  }
  const classOffset = cssModule.code.indexOf(".sw_");
  expect(classOffset).toBeGreaterThanOrEqual(0);
  const original = originalPositionFor(
    new TraceMap(JSON.stringify(cssModule.map)),
    positionAt(cssModule.code, classOffset),
  );

  expect(original).toMatchObject({
    source: expect.stringMatching(/\/entry\.ts$/),
    ...positionAt(source, source.indexOf("style('red'")),
  });
});

describe("maps after removing a macro import and adding CSS imports", () => {
  test("inserted CSS imports have no position in the original source", () => {
    const { map } = createImportReplacementMap();

    expect(originalPositionFor(map, { line: 1, column: 0 }).source).toBeNull();
  });

  test("the generated props object maps to the original style call", () => {
    const { source, generated, map } = createImportReplacementMap();

    const original = originalPositionFor(
      map,
      positionAt(generated, generated.indexOf("className")),
    );

    expect(original).toEqual({
      source: "/src/a.tsx",
      ...positionAt(source, source.indexOf('style("flex")')),
      name: null,
    });
    expect(map.sourcesContent?.[0]).toBe(source);
  });

  test.each([
    { location: "code after the style call", text: "log(a)" },
    { location: "code after an emoji", text: '";\r\nconst a' },
  ])("$location keeps its original position", ({ text }) => {
    const { source, generated, map } = createImportReplacementMap();

    const original = originalPositionFor(map, positionAt(generated, generated.indexOf(text)));

    expect(original).toEqual({
      source: "/src/a.tsx",
      ...positionAt(source, source.indexOf(text)),
      name: null,
    });
  });
});

test("a Vue production template's generated class maps to its original style call", async () => {
  const root = createProjectDirectory();
  const source = `<script setup lang="ts">
import { style } from 'stillwind/vue';
</script>
<template>
  <p v-bind="style('red')">日本語 🧭</p>
</template>`;
  writeFileSync(path.join(root, "root.css"), "@utility red {color:red} @stillwind-slot utilities;");
  writeFileSync(path.join(root, "Component.vue"), source);

  const result = await build({
    configFile: false,
    root,
    logLevel: "silent",
    plugins: [
      stillwind({
        css: path.join(root, "root.css"),
      }),
      vue(),
    ],
    build: {
      write: false,
      sourcemap: true,
      minify: false,
      lib: {
        entry: path.join(root, "Component.vue"),
        formats: ["es"],
      },
      rollupOptions: {
        external: ["vue"],
      },
    },
  });
  const bundle = Array.isArray(result) ? result[0] : result;
  if (!("output" in bundle)) {
    expect.fail("Expected a completed Vue build");
  }
  const classPositions = bundle.output.flatMap((chunk) => {
    if (chunk.type !== "chunk" || !chunk.map) {
      return [];
    }
    const map = new TraceMap(chunk.map.toString());
    return [...chunk.code.matchAll(/sw_[0-9a-z]+/g)].map((match) =>
      originalPositionFor(map, positionAt(chunk.code, match.index)),
    );
  });

  expect(classPositions).toContainEqual({
    source: expect.stringMatching(/Component\.vue$/),
    ...positionAt(source, source.indexOf("style('red')")),
    name: null,
  });
});

function createProjectDirectory() {
  const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../../..");
  const root = mkdtempSync(path.join(repositoryRoot, ".test-source-map-"));
  onTestFinished(() => rmSync(root, { recursive: true, force: true }));
  return root;
}

async function createPreprocessedProject() {
  const root = createProjectDirectory();
  const source = `import { style } from 'stillwind/react';
// 日本語 🧭
export const props = style('red', { '--x': progress },
  { '--value': data },
  { '--y': progress });`;
  const entryFile = path.join(root, "entry.ts");
  writeFileSync(path.join(root, "root.css"), "@utility red {color:red} @stillwind-slot utilities;");
  writeFileSync(entryFile, source);
  const server = await createServer({
    configFile: false,
    root,
    logLevel: "silent",
    resolve: {
      alias: {
        "stillwind/runtime/class-name-object": fileURLToPath(
          new URL("../../../core/src/runtime/class-name-object.ts", import.meta.url),
        ),
      },
    },
    plugins: [
      {
        name: "framework-preprocessing-fixture",
        enforce: "pre",
        transform(code, id) {
          if (id !== entryFile.replaceAll("\\", "/")) {
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
      }),
    ],
    css: {
      devSourcemap: true,
    },
    server: {
      middlewareMode: true,
      watch: null,
    },
  });
  onTestFinished(() => server.close());
  return {
    server,
    source,
  };
}

function createImportReplacementMap() {
  const macroImport = 'import { style } from "stillwind/react";\r\n';
  const macroCall = 'style("flex")';
  const source = macroImport + 'const emoji = "😀";\r\nconst a = style("flex");\r\nconsole.log(a);';
  const callStart = source.indexOf(macroCall);
  const props = '{ className: "_s_example" }';
  const compiled = source.slice(0, callStart) + props + source.slice(callStart + macroCall.length);
  const cssImports = 'import "base.css";\nimport "module.css";\n';
  const map = new TraceMap(
    moduleSourceMap(
      source,
      compiled,
      "/src/a.tsx",
      [
        {
          originalStart: callStart,
          originalEnd: callStart + macroCall.length,
          generatedStart: callStart,
          generatedEnd: callStart + props.length,
        },
      ],
      [
        {
          from: 0,
          to: macroImport.length,
        },
      ],
      cssImports,
    ),
  );
  return { source, generated: cssImports + compiled.slice(macroImport.length), map };
}

function positionAt(text: string, offset: number) {
  const prefix = text.slice(0, offset);
  return {
    line: prefix.split("\n").length,
    column: offset - prefix.lastIndexOf("\n") - 1,
  };
}
