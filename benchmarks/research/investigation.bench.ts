import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { resolveFixtureCss } from "@stillwind-internal/tools-fixture-css/stillwind-css";
import { compileModule, fileModuleDescriptor, TypeScriptFactsService } from "@stillwind/host";
import { NativeDesignSystemLoader, NativeProjectSession } from "@stillwind/native";
import { expect, test, type BenchRunOptions } from "vite-plus/test";

import { cssWithSourceMap, decodeCssSourceMap } from "../../packages/host/src/css-source-map.ts";
import { cssWithSourceMap as candidate } from "../source-map/candidate.ts";
import { cssInput } from "../source-map/input.ts";

const options: BenchRunOptions = {
  iterations: 15,
  time: 0,
  warmupIterations: 5,
  warmupTime: 0,
};

for (const keys of [0, 8, 64, 256]) {
  test(`source-map/replacement-count/${keys}`, async ({ bench, task }) => {
    const { output } = cssInput(1000);
    const replacements = new Map(
      Array.from({ length: keys }, (_, i) => [
        `__stillwind_resource_${i.toString(16).padStart(64, "0")}__`,
        `/assets/${i}.svg`,
      ]),
    );
    let productionResult = "";
    let candidateResult = "";
    task.meta.measurement = options;
    await bench.compare(
      bench("production", { async: false }, () => {
        productionResult = cssWithSourceMap(output, replacements);
      }),
      bench("candidate", { async: false }, () => {
        candidateResult = candidate(output, replacements);
      }),
      options,
    );
    expect(decodeCssSourceMap(candidateResult)).toStrictEqual(decodeCssSourceMap(productionResult));
  });
}

for (const exact of [false, true]) {
  for (const includeSourcesContent of [false, true]) {
    test(`source-map/10000/exact=${exact}/content=${includeSourcesContent}`, async ({
      bench,
      task,
    }) => {
      const { output, replacements } = cssInput(10_000, exact);
      let productionResult = "";
      let candidateResult = "";
      task.meta.measurement = options;
      await bench.compare(
        bench("production", { async: false }, () => {
          productionResult = cssWithSourceMap(
            output,
            replacements,
            undefined,
            includeSourcesContent,
          );
        }),
        bench("candidate", { async: false }, () => {
          candidateResult = candidate(output, replacements, undefined, includeSourcesContent);
        }),
        options,
      );
      expect(decodeCssSourceMap(candidateResult)).toStrictEqual(
        decodeCssSourceMap(productionResult),
      );
    });
  }
}

const root = fileURLToPath(new URL("../", import.meta.url));
const loader = new NativeDesignSystemLoader();
loader.provide(
  0,
  "root.css",
  "@utility red {color:red;} @utility blue {color:blue;} @stillwind-slot utilities;",
  "untitled:root.css",
  "untitled:root.css",
);
const semantic =
  "import {style} from 'stillwind/react'; const token=style.keyframes({from:'red',to:'blue'}); const selected=token; export const props=style(selected(''));";
const host = {
  describe: (id: string) => fileModuleDescriptor(root, id),
  resolve: async () => null,
  read: async () => expect.fail("Unexpected dependency read"),
  watch: () => {},
  warn: () => {},
};

for (const operation of ["unchanged", "edit"] as const) {
  test(`host/phases/${operation}`, async ({ bench, task }) => {
    const samples: Array<{
      checkerMs: number;
      nativeMs: number;
      totalMs: number;
    }> = [];
    const session = new NativeProjectSession(loader, true);
    const facts = new TypeScriptFactsService(root);
    const originalCollect = facts.collect.bind(facts);
    const originalCompile = session.compile.bind(session);
    let checkerMs = 0;
    let nativeMs = 0;
    facts.collect = (...args) => {
      const start = performance.now();
      try {
        return originalCollect(...args);
      } finally {
        checkerMs += performance.now() - start;
      }
    };
    session.compile = (...args) => {
      const start = performance.now();
      try {
        return originalCompile(...args);
      } finally {
        nativeMs += performance.now() - start;
      }
    };
    let edit = 0;
    let output: Awaited<ReturnType<typeof compileModule>>;
    try {
      const analyze = (source: string) =>
        compileModule(
          session,
          () => facts,
          source,
          path.join(root, "perf-semantic.tsx"),
          root,
          host,
        );
      await analyze(semantic);
      task.meta.measurement = options;
      await bench(operation, { async: true }, async () => {
        checkerMs = 0;
        nativeMs = 0;
        const start = performance.now();
        output = await analyze(
          operation === "edit" && ++edit % 2 ? semantic.replaceAll("red", "blue") : semantic,
        );
        samples.push({
          checkerMs,
          nativeMs,
          totalMs: performance.now() - start,
        });
      }).run(options);
      expect(output!.diagnostics).toEqual([]);
      task.meta.notes = JSON.stringify({
        phases: samples.slice(options.warmupIterations),
        stats: facts.stats,
      });
    } finally {
      facts.close();
    }
  });
}

test("host/fresh/phases", async ({ bench, task }) => {
  const samples: Array<{
    createMs: number;
    compileMs: number;
    closeMs: number;
  }> = [];
  let output: Awaited<ReturnType<typeof compileModule>>;
  task.meta.measurement = options;
  await bench("fresh", { async: true }, async () => {
    const start = performance.now();
    const facts = new TypeScriptFactsService(root);
    const created = performance.now();
    try {
      output = await compileModule(
        new NativeProjectSession(loader, true),
        () => facts,
        semantic,
        path.join(root, "perf-semantic.tsx"),
        root,
        host,
      );
      const compiled = performance.now();
      facts.close();
      samples.push({
        createMs: created - start,
        compileMs: compiled - created,
        closeMs: performance.now() - compiled,
      });
    } finally {
      facts.close();
    }
  }).run(options);
  expect(output!.diagnostics).toEqual([]);
  task.meta.notes = JSON.stringify({
    phases: samples.slice(options.warmupIterations),
  });
});

for (const [name, css] of [
  ["minimal", "@utility flex {display:flex;} @stillwind-slot utilities;"],
  ["full-registry", resolveFixtureCss('@import "stillwind";')],
] as const) {
  test(`compiler/reused-loader/${name}`, async ({ bench, task }) => {
    const design = new NativeDesignSystemLoader();
    design.provide(0, "root.css", css, "untitled:root.css", "untitled:root.css");
    const session = new NativeProjectSession(design, true);
    const sources = [
      {
        kind: "direct" as const,
        moduleId: "loader.tsx",
        document: {
          uri: "untitled:loader.tsx",
          sourceMapSource: "untitled:loader.tsx",
          format: "jsx" as const,
          code: "import {style} from 'stillwind/react'; export const props=style('flex');",
        },
        syntax: {
          language: "tsx" as const,
          moduleKind: "unambiguous" as const,
        },
      },
    ];
    session.compile(sources);
    let output: ReturnType<NativeProjectSession["compile"]>;
    task.meta.measurement = options;
    await bench(name, { async: false }, () => {
      output = session.compile(sources);
    }).run(options);
    expect(output!.modules[0].diagnostics).toEqual([]);
  });
}

for (const count of [10, 100]) {
  for (const operation of ["unchanged", "edit"] as const) {
    test(`host/graph/${count}/${operation}`, async ({ bench, task }) => {
      const graphRoot = mkdtempSync(path.join(root, ".test-research-graph-"));
      const service = new TypeScriptFactsService(graphRoot);
      const session = new NativeProjectSession(loader, true);
      let reads = 0;
      let resolves = 0;
      const graphHost = {
        ...host,
        describe: (id: string) => fileModuleDescriptor(graphRoot, id),
        resolve: async (specifier: string, importer: string) => {
          resolves++;
          return specifier.startsWith(".")
            ? { id: path.resolve(path.dirname(importer), specifier + ".ts") }
            : null;
        },
        read: async (id: string) => {
          reads++;
          return readFileSync(id, "utf8");
        },
      };
      const source =
        "import {style} from 'stillwind/react';\n" +
        Array.from(
          { length: count },
          (_, i) => `import {token as token${i}} from './producer-${i}';`,
        ).join("\n") +
        "\n" +
        Array.from(
          { length: count },
          (_, i) => `export const props${i}=style(token${i}(''));`,
        ).join("\n");
      for (let i = 0; i < count; i++) {
        writeFileSync(
          path.join(graphRoot, `producer-${i}.ts`),
          "import {style} from 'stillwind/react'; export const token=style.keyframes({from:'red',to:'blue'});",
        );
      }
      const id = path.join(graphRoot, "entry.tsx");
      writeFileSync(id, source);
      let output: Awaited<ReturnType<typeof compileModule>>;
      let edit = 0;
      const graphOptions = { ...options, iterations: 5, warmupIterations: 1 };
      try {
        await compileModule(session, () => service, source, id, graphRoot, graphHost);
        reads = 0;
        resolves = 0;
        task.meta.measurement = graphOptions;
        await bench(operation, { async: true }, async () => {
          const text = operation === "edit" ? source + `\n// edit ${++edit}` : source;
          output = await compileModule(session, () => service, text, id, graphRoot, graphHost);
        }).run(graphOptions);
        expect(output!.diagnostics).toEqual([]);
        task.meta.notes = JSON.stringify({
          reads,
          resolves,
          stats: service.stats,
          sources: count + 1,
        });
      } finally {
        service.close();
        rmSync(graphRoot, { recursive: true, force: true });
      }
    });
  }
}
