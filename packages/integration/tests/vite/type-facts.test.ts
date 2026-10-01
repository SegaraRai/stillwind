import { readFileSync, mkdtempSync, writeFileSync, rmSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { TypeScriptFactsService, type ModuleSummary } from "@stillwind/host";
import { stillwind } from "@stillwind/integration/vite";
import {
  compileProject,
  foldCalcExpressions,
  summarizeSource,
  NativeDesignSystemLoader,
  NativeProjectSession,
} from "@stillwind/native";
import { resolveFixtureCss } from "@stillwind/tools/stillwind-css";
import { build } from "vite";
import { expect, test } from "vite-plus/test";

import { finalizeModule } from "../../src/vite/finalize.ts";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../../..");
const fixtureRoot = path.join(path.dirname(fileURLToPath(import.meta.url)), "fixtures/type-facts");
const cssInput = (content: string) => ({
  content,
  name: "root.css",
  uri: "untitled:root.css",
  sourceMapSource: "untitled:root.css",
});

test("Vite aliases select the same asset in the host and checker", async () => {
  const root = mkdtempSync(path.join(path.dirname(fixtureRoot), "../../.test-session-"));
  const put = (name: string, text: string) => writeFileSync(path.join(root, name), text);
  put("index.html", '<script type="module" src="/entry.ts"></script>');
  put(
    "root.css",
    "@utility chosen {color:rgb(11,22,33)} @stillwind-slot utilities; @stillwind-slot keyframes;",
  );
  put(
    "asset.ts",
    "import {style} from 'stillwind/react'; export const token=style.keyframes({from:'chosen',to:'chosen'});",
  );
  put(
    "entry.ts",
    "import {style} from 'stillwind/react'; import {token} from '@selected'; console.log(style(token('')));",
  );
  try {
    const result = await build({
      configFile: false,
      root,
      logLevel: "silent",
      resolve: { alias: { "@selected": path.join(root, "asset.ts") } },
      plugins: [stillwind({ css: path.join(root, "root.css"), types: false })],
      build: { write: false, minify: false, cssMinify: false },
    });
    if (!("output" in result)) {
      expect.fail('Unexpected value: "output" in result');
    }
    const css = result.output
      .flatMap((asset) =>
        asset.type === "asset" && asset.fileName.endsWith(".css") ? [String(asset.source)] : [],
      )
      .join("\n");
    expect(css).toMatch(/@keyframes/);
    expect(css).toMatch(/11,\s*22,\s*33/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

const sources = ["producer.ts", "chooser.ts", "barrel.ts", "consumer.ts"]
  .map((name) => ({
    resolverId: path.join(fixtureRoot, name),
    checkerFileName: path.join(fixtureRoot, name),
    projectionFileName: `src/${name}`,
    moduleId: `src/${name}`,
    kind: "direct" as const,
    document: {
      uri: `untitled:src/${name}`,
      code: readFileSync(path.join(fixtureRoot, name), "utf8"),
      format: "script" as const,
      sourceMapSource: `untitled:src/${name}`,
    },
    syntax: {
      language: "ts" as const,
      moduleKind: "unambiguous" as const,
    },
  }))
  .map((source) => ({
    ...source,
    probes: (
      summarizeSource(source.document.code, source.moduleId, "tsx", "unambiguous") as ModuleSummary
    ).typeProbes,
  }));

const summaries = new Map(
  sources.map((source) => [
    source.moduleId,
    summarizeSource(source.document.code, source.moduleId, "tsx", "unambiguous") as ModuleSummary,
  ]),
);

test("TypeScript is required for cross-module conditional ref propagation", () => {
  const withoutFacts = compileProject(
    sources.map(({ moduleId: sourcePath, document, syntax }) => ({
      kind: "direct",
      moduleId: sourcePath,
      document,
      syntax,
    })),
    cssInput(resolveFixtureCss('@import "stillwind";')),
  ) as { modules: Array<{ moduleId: string; diagnostics: Array<{ code: string }> }> };
  expect(
    withoutFacts.modules.find((module) => module.moduleId === "src/consumer.ts")?.diagnostics
      .length,
    "native compilation must not implement cross-module propagation on its own",
  ).toBeTruthy();

  const service = new TypeScriptFactsService(repositoryRoot);
  const assets = new Map(
    [...summaries].map(([sourcePath, summary]) => [sourcePath, summary.assets] as const),
  );
  service.collect(sources, assets);
  const facts = service.collect(
    sources.map((source) =>
      source.moduleId === "src/consumer.ts"
        ? {
            ...source,
            document: {
              ...source.document,
              code: `${source.document.code}\n`,
            },
          }
        : source,
    ),
    assets,
  );
  const { overlayReads, diskReadRequests, ...lifecycle } = service.stats;
  expect(overlayReads).toBeGreaterThan(0);
  expect(diskReadRequests).toBeGreaterThan(0);
  expect(lifecycle).toStrictEqual({
    starts: 1,
    snapshots: 2,
    incrementalSnapshots: 1,
    restarts: 0,
  });
  service.close();
  const withFacts = compileProject(
    sources.map(({ moduleId: sourcePath, document, syntax }) => ({
      kind: "direct",
      moduleId: sourcePath,
      document,
      syntax,
      typeFacts: facts.get(sourcePath) ?? [],
    })),
    cssInput(resolveFixtureCss('@import "stillwind";')),
  ) as {
    modules: Array<{
      moduleId: string;
      code: string;
      css: string;
      diagnostics: Array<{ code: string }>;
    }>;
  };
  const consumer = withFacts.modules.find((module) => module.moduleId === "src/consumer.ts");
  expect(consumer).toBeTruthy();
  expect(consumer!.diagnostics).toStrictEqual([]);
  expect(consumer!.code).toMatch(/\(\(selected\)\.c\)/);
  // Only the attachment supplies the selected marker.
  expect(consumer!.code).toMatch(/attached = \{className:\(\(selected\)\.c\)\}/);
  // A dynamic animation selects its prebuilt class by the keyframes name the value carries.
  expect(consumer!.code).toMatch(/\[\(selectedKeyframes\)\.k\]/);
  // A transition the value selects at runtime carries one class per token, chosen by v.
  expect(consumer!.code).toMatch(
    /"sw-vt-[^"]+":"sw_[^"]+"[\s\S]{0,120}?\)\[\(selectedTransition\)\.v\]/,
  );
  expect(consumer!.code).toMatch(/stillwindViewTransitionName\("sw-vt-[^"]+",transitionKey\)/);
  expect(consumer!.code).toMatch(/stillwindViewTransitionName\(selectedTransition,transitionKey\)/);
  expect(consumer!.code).toMatch(
    /stillwindViewTransitionName\(transitionData\.transition,transitionData\.key\)/,
  );
  expect(consumer!.code.match(/transitionKey/g)?.length).toBe(4);
  expect(consumer!.css.match(/anchor-name:--sw-a-/g)?.length).toBe(2);
  expect(consumer!.css.match(/animation-name:sw-kf-/g)?.length).toBe(2);
  expect(consumer!.css).toMatch(/view-transition-class:sw-vt-/);
  const loader = new NativeDesignSystemLoader();
  loader.provide(
    0,
    "fixture.css",
    resolveFixtureCss('@import "stillwind";'),
    "untitled:fixture.css",
    "untitled:fixture.css",
  );
  const session = new NativeProjectSession(loader, true);
  const selected = session.compile(
    sources.map((source) => ({
      ...source,
      typeFacts: facts.get(source.moduleId) ?? [],
    })),
    "src/consumer.ts",
  ) as typeof withFacts;
  expect(selected.modules.map((module) => module.moduleId)).toStrictEqual(["src/consumer.ts"]);
  expect(selected.modules[0].diagnostics).toStrictEqual([]);
  expect(session.moduleCss("src/consumer.ts")).toMatch(/animation-name:sw-kf-/);
  const selection: import("@stillwind/native").NativeStylesheetSelection = {
    modules: ["src/consumer.ts"],
    coverage: { kind: "complete", target: "consumer-route", revision: "graph-1" },
    placement: "carrier",
  };
  const plan = session.planStylesheet(selection) as {
    resources: { moduleId: string; specifier: string }[];
    css: string;
    coverage: typeof selection.coverage;
    producers: { module: string; sourceRevision: string; semanticRevision: string }[];
  };
  const reference = session.planStylesheet({ ...selection, coverage: { kind: "observed" } });
  expect(reference.css).toBe(session.moduleCss("src/consumer.ts"));
  expect(plan.css).toMatch(/animation-iteration-count:infinite/);
  expect(plan.css).toMatch(/@keyframes sw-kf-/);
  expect(plan.css.match(/@keyframes /g)?.length).toBe(2);
  expect(plan.css).not.toMatch(/display:flex/);
  expect(plan.resources.map(({ moduleId, specifier }) => ({ moduleId, specifier }))).toStrictEqual([
    { moduleId: "src/producer.ts", specifier: "./support.svg" },
  ]);
  expect(plan.coverage).toStrictEqual(selection.coverage);
  expect(plan.producers.map((producer) => producer.module)).toStrictEqual(selection.modules);
  expect(plan.producers[0].sourceRevision).toMatch(/^[a-f0-9]{64}$/);
  expect(plan.producers[0].semanticRevision).toMatch(/^[a-f0-9]{64}$/);
  expect(() =>
    session.planStylesheet({
      ...selection,
      modules: [...selection.modules, "src/unvisited.ts"],
    }),
  ).toThrow(/unregistered modules/);
  expect(session.moduleCss("src/producer.ts")).toMatch(/display:flex/);
  session.compile(
    sources.map((source) => ({
      ...source,
      document: {
        ...source.document,
        code:
          source.moduleId === "src/producer.ts"
            ? source.document.code.replaceAll("opacity-0", "opacity-5")
            : source.document.code,
      },
      typeFacts: facts.get(source.moduleId) ?? [],
    })),
    "src/consumer.ts",
  );
  const changedPlan = session.planStylesheet(selection) as typeof plan;
  expect(changedPlan.producers[0].semanticRevision).not.toBe(plan.producers[0].semanticRevision);
  // CSS values may retain calc() until the bundler's optimization stage.
  expect(foldCalcExpressions(changedPlan.css, undefined)).toMatch(/opacity:5%/);

  const runnable = finalizeModule(consumer!.code, false);
  expect(runnable).not.toMatch(/from "stillwind\/react"/);
  expect(runnable).toMatch(/^import \{[^}]+\} from "stillwind\/react\/runtime";/);
  for (const helper of runnable.matchAll(/\b(stillwind[A-Z]\w*)\s*\(/g)) {
    expect(runnable).toMatch(new RegExp(`import \\{[^}]*\\b${helper[1]}\\b[^}]*\\}`));
  }
});
