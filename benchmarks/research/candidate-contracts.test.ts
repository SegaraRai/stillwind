import { cssWithSourceMap, decodeCssSourceMap } from "@stillwind/host";
import { NativeDesignSystemLoader, NativeProjectSession } from "@stillwind/native";
import { expect, test } from "vite-plus/test";

import { cssWithSourceMap as candidate } from "../source-map/candidate.ts";

test("candidate matches native plans with authored CSS, several modules and resources", () => {
  const loader = new NativeDesignSystemLoader();
  const loaded = loader.provide(
    0,
    "root.css",
    '/* 日本語 🧭 */ .authored{color:green} @utility red {color:red;} @utility picture {background-image:url("./photo.svg");} @stillwind-slot utilities;',
    "untitled:root.css",
    "untitled:root.css",
  );
  if (loaded.kind !== "complete") {
    expect.fail("Expected complete design-system load");
  }
  const session = new NativeProjectSession(loader, true);
  const sources = Array.from({ length: 12 }, (_, i) => ({
    kind: "direct" as const,
    moduleId: `map-${i}.tsx`,
    document: {
      uri: `untitled:map-${i}.tsx`,
      sourceMapSource: `untitled:map-${i}.tsx`,
      format: "jsx" as const,
      code: `import {style} from 'stillwind/react';\n// 日本語 🧭\nexport const props=style('${i % 2 ? "red picture" : "picture red"}');`,
    },
    syntax: { language: "tsx" as const, moduleKind: "unambiguous" as const },
  }));
  expect(session.compile(sources).modules.every((module) => module.diagnostics.length === 0)).toBe(
    true,
  );
  for (const coverage of [
    { kind: "observed" as const },
    { kind: "complete" as const, target: "document", revision: "research" },
  ]) {
    const output = session.planStylesheet({
      modules: sources.map((source) => source.moduleId),
      coverage,
      placement: "document",
    });
    const replacements = new Map(
      [...loaded.resources, ...output.resources].map((resource) => [
        resource.placeholder,
        "/日本語/image.svg",
      ]),
    );
    expect(output.sourceMap.sources.length).toBeGreaterThan(1);
    expect(replacements.size).toBeGreaterThan(0);
    for (const includeSourcesContent of [true, false]) {
      const production = decodeCssSourceMap(
        cssWithSourceMap(output, replacements, undefined, includeSourcesContent),
      );
      const result = decodeCssSourceMap(
        candidate(output, replacements, undefined, includeSourcesContent),
      );
      expect(result).toStrictEqual(production);
      expect(result.code).toContain("/日本語/image.svg");
    }
  }
});

test("candidate retains mappings across varied Unicode origins and placeholder prefixes", () => {
  for (let i = 0; i < 128; i++) {
    const text = `${"🧭日\r\n".repeat(i % 7)}.x{color:FOO_LONG;background:FOO}.y{content:'${i}';}`;
    const start = text.indexOf(".x");
    const end = text.indexOf(".y");
    const output = {
      css: `/* ${i} */` + text.slice(start),
      sourceMap: {
        sources: [{ sourceMapSource: `untitled:unicode-${i}.css`, content: text }],
        entries: [
          {
            source: 0,
            generatedStart: Buffer.byteLength(`/* ${i} */`),
            generatedEnd: Buffer.byteLength(`/* ${i} */` + text.slice(start, end)),
            originalStart: Buffer.byteLength(text.slice(0, start)),
            originalEnd: Buffer.byteLength(text.slice(0, end)),
            exact: i % 2 === 0,
          },
        ],
      },
    };
    const replacements = new Map(
      i % 3
        ? [
            ["FOO_LONG", "🧭\nblue"],
            ["FOO", "red"],
          ]
        : [
            ["FOO", "red"],
            ["FOO_LONG", "🧭\nblue"],
          ],
    );
    expect(decodeCssSourceMap(candidate(output, replacements))).toStrictEqual(
      decodeCssSourceMap(cssWithSourceMap(output, replacements)),
    );
  }
});
