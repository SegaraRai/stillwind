import assert from "node:assert/strict";
import { readFileSync, mkdtempSync, writeFileSync, rmSync } from "node:fs";
import path from "node:path";
import test from "node:test";
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
import { build } from "vite";

import { resolveFixtureCss } from "../../../../tools/lib/stillwind-fixture-css.ts";
import { finalizeModule } from "../../src/vite/finalize.ts";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../../..");
const fixtureRoot = path.join(path.dirname(fileURLToPath(import.meta.url)), "fixtures/type-facts");
const cssInput = (content: string) => ({
  content,
  name: "root.css",
  uri: "untitled:root.css",
  sourceMapSource: "untitled:root.css",
});

void test("Vite aliases select the same asset in the host and checker", async () => {
  const root = mkdtempSync(path.join(path.dirname(fixtureRoot), "../.session-test-"));
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
    assert.ok("output" in result);
    const css = result.output
      .flatMap((asset) =>
        asset.type === "asset" && asset.fileName.endsWith(".css") ? [String(asset.source)] : [],
      )
      .join("\n");
    assert.match(css, /@keyframes/);
    assert.match(css, /11,\s*22,\s*33/);
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

void test("TypeScript is required for cross-module conditional ref propagation", () => {
  const withoutFacts = compileProject(
    sources.map(({ moduleId: sourcePath, document, syntax }) => ({
      kind: "direct",
      moduleId: sourcePath,
      document,
      syntax,
    })),
    cssInput(resolveFixtureCss('@import "stillwind";')),
  ) as { modules: Array<{ moduleId: string; diagnostics: Array<{ code: string }> }> };
  assert(
    withoutFacts.modules.find((module) => module.moduleId === "src/consumer.ts")?.diagnostics
      .length,
    "native compilation must not implement cross-module propagation on its own",
  );

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
  assert.ok(overlayReads > 0);
  assert.ok(diskReadRequests > 0);
  assert.deepEqual(lifecycle, {
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
  assert(consumer);
  assert.deepEqual(consumer.diagnostics, []);
  assert.match(consumer.code, /\(\(selected\)\.c\)/);
  // Only the attachment supplies the selected marker.
  assert.match(consumer.code, /attached = \{className:\(\(selected\)\.c\)\}/);
  // A dynamic animation selects its prebuilt class by the keyframes name the value carries.
  assert.match(consumer.code, /\[\(selectedKeyframes\)\.k\]/);
  // A transition the value selects at runtime carries one class per token, chosen by v.
  assert.match(
    consumer.code,
    /"sw-vt-[^"]+":"sw_[^"]+"[\s\S]{0,120}?\)\[\(selectedTransition\)\.v\]/,
  );
  assert.match(consumer.code, /stillwindViewTransitionName\("sw-vt-[^"]+",transitionKey\)/);
  assert.match(consumer.code, /stillwindViewTransitionName\(selectedTransition,transitionKey\)/);
  assert.match(
    consumer.code,
    /stillwindViewTransitionName\(transitionData\.transition,transitionData\.key\)/,
  );
  assert.equal(consumer.code.match(/transitionKey/g)?.length, 4);
  assert.equal(consumer.css.match(/anchor-name:--sw-a-/g)?.length, 2);
  assert.equal(consumer.css.match(/animation-name:sw-kf-/g)?.length, 2);
  assert.match(consumer.css, /view-transition-class:sw-vt-/);
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
  assert.deepEqual(
    selected.modules.map((module) => module.moduleId),
    ["src/consumer.ts"],
  );
  assert.deepEqual(selected.modules[0].diagnostics, []);
  assert.match(session.moduleCss("src/consumer.ts"), /animation-name:sw-kf-/);
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
  assert.equal(reference.css, session.moduleCss("src/consumer.ts"));
  assert.match(plan.css, /animation-iteration-count:infinite/);
  assert.match(plan.css, /@keyframes sw-kf-/);
  assert.equal(plan.css.match(/@keyframes /g)?.length, 2);
  assert.doesNotMatch(plan.css, /display:flex/);
  assert.deepEqual(
    plan.resources.map(({ moduleId, specifier }) => ({ moduleId, specifier })),
    [{ moduleId: "src/producer.ts", specifier: "./support.svg" }],
  );
  assert.deepEqual(plan.coverage, selection.coverage);
  assert.deepEqual(
    plan.producers.map((producer) => producer.module),
    selection.modules,
  );
  assert.match(plan.producers[0].sourceRevision, /^[a-f0-9]{64}$/);
  assert.match(plan.producers[0].semanticRevision, /^[a-f0-9]{64}$/);
  assert.throws(
    () =>
      session.planStylesheet({
        ...selection,
        modules: [...selection.modules, "src/unvisited.ts"],
      }),
    /unregistered modules/,
  );
  assert.match(session.moduleCss("src/producer.ts"), /display:flex/);
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
  assert.notEqual(changedPlan.producers[0].semanticRevision, plan.producers[0].semanticRevision);
  // CSS values may retain calc() until the bundler's optimization stage.
  assert.match(foldCalcExpressions(changedPlan.css, undefined), /opacity:5%/);

  const runnable = finalizeModule(consumer.code, false);
  assert.doesNotMatch(runnable, /from "stillwind\/react"/);
  assert.match(runnable, /^import \{[^}]+\} from "stillwind\/react\/runtime";/);
  for (const helper of runnable.matchAll(/\b(stillwind[A-Z]\w*)\s*\(/g)) {
    assert.match(runnable, new RegExp(`import \\{[^}]*\\b${helper[1]}\\b[^}]*\\}`));
  }
});
