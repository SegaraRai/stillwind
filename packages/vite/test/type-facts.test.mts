import assert from "node:assert/strict";
import { readFileSync, mkdtempSync, writeFileSync, rmSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { TypeScriptFactsService, type ModuleSummary } from "@stillwind/host";
import {
  compileReactProject,
  foldCalcExpressions,
  summarizeReact,
  NativeDesignSystemLoader,
  NativeProjectSession,
} from "@stillwind/native";
import { build } from "vite-plus";

import { resolveFixtureCss } from "../../../tools/lib/stillwind-fixture-css.ts";
import { finalizeReactModule, stillwind } from "../src/index.ts";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../..");
const fixtureRoot = path.join(path.dirname(fileURLToPath(import.meta.url)), "fixtures/type-facts");

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
    "import {style} from 'stillwind/react'; export const token=style.animation({from:'chosen',to:'chosen'});",
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
    id: path.join(fixtureRoot, name),
    path: `src/${name}`,
    source: readFileSync(path.join(fixtureRoot, name), "utf8"),
  }))
  .map((source) => ({
    ...source,
    probes: (summarizeReact(source.source, source.path) as ModuleSummary).type_probes,
  }));

const summaries = new Map(
  sources.map((source) => [
    source.path,
    summarizeReact(source.source, source.path) as ModuleSummary,
  ]),
);

void test("TypeScript is required for cross-module conditional handle propagation", () => {
  const withoutFacts = compileReactProject(
    sources.map(({ path: sourcePath, source }) => ({ path: sourcePath, source })),
    resolveFixtureCss('@import "stillwind";'),
  ) as { modules: Array<{ path: string; diagnostics: Array<{ code: string }> }> };
  assert(
    withoutFacts.modules.find((module) => module.path === "src/consumer.ts")?.diagnostics.length,
    "native compilation must not implement cross-module propagation on its own",
  );

  const service = new TypeScriptFactsService(repositoryRoot);
  const assets = new Map(
    [...summaries].map(([sourcePath, summary]) => [sourcePath, summary.assets] as const),
  );
  service.collect(sources, assets);
  const facts = service.collect(
    sources.map((source) =>
      source.path === "src/consumer.ts" ? { ...source, source: `${source.source}\n` } : source,
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
  const withFacts = compileReactProject(
    sources.map(({ path: sourcePath, source }) => ({
      path: sourcePath,
      source,
      type_facts: facts.get(sourcePath) ?? [],
    })),
    resolveFixtureCss('@import "stillwind";'),
  ) as {
    modules: Array<{
      path: string;
      code: string;
      css: string;
      diagnostics: Array<{ code: string }>;
    }>;
  };
  const consumer = withFacts.modules.find((module) => module.path === "src/consumer.ts");
  assert(consumer);
  assert.deepEqual(consumer.diagnostics, []);
  assert.match(consumer.code, /stillwindHandleClass\(selected\)/);
  assert.match(consumer.code, /stillwindAssetClass\(selectedAnimation\("infinite"\)/);
  assert.match(consumer.code, /stillwindViewTransitionStyle\(selectedTransition,undefined/);
  assert.match(consumer.code, /stillwindViewTransitionStyle\("--sw-v-[^"]+",transitionKey/);
  assert.match(consumer.code, /stillwindViewTransitionStyle\(selectedTransition,transitionKey/);
  assert.match(consumer.code, /stillwindViewTransitionStyle\(keyedTransitionAlias,undefined/);
  assert.equal(consumer.code.match(/transitionKey/g)?.length, 4);
  assert.equal(consumer.css.match(/anchor-name:--sw-r-/g)?.length, 2);
  assert.equal(consumer.css.match(/animation-name:--sw-a-/g)?.length, 2);
  assert.match(consumer.css, /view-transition-name:var\(--sw-vt-name\)/);
  assert.match(consumer.css, /view-transition-class:var\(--sw-vt-class\)/);
  const loader = new NativeDesignSystemLoader();
  loader.provide(0, "fixture.css", resolveFixtureCss('@import "stillwind";'));
  const session = new NativeProjectSession(loader, true);
  const selected = session.compile(
    sources.map((source) => ({
      ...source,
      type_facts: facts.get(source.path) ?? [],
    })),
    "src/consumer.ts",
  ) as typeof withFacts;
  assert.deepEqual(
    selected.modules.map((module) => module.path),
    ["src/consumer.ts"],
  );
  assert.deepEqual(selected.modules[0].diagnostics, []);
  assert.match(session.moduleCss("src/consumer.ts"), /animation-name:--sw-a-/);
  const selection: import("@stillwind/native").NativeStylesheetSelection = {
    modules: ["src/consumer.ts"],
    coverage: { kind: "complete", target: "consumer-route", revision: "graph-1" },
    placement: "carrier",
  };
  const plan = session.planStylesheet(selection) as {
    resources: { owner: string; specifier: string }[];
    css: string;
    coverage: typeof selection.coverage;
    producers: { module: string; sourceRevision: string; semanticRevision: string }[];
  };
  assert.equal(plan.css, session.moduleCss("src/consumer.ts"));
  assert.match(plan.css, /@keyframes --sw-a-/);
  assert.equal(plan.css.match(/@keyframes /g)?.length, 2);
  assert.doesNotMatch(plan.css, /display:flex/);
  assert.deepEqual(
    plan.resources.map(({ owner, specifier }) => ({ owner, specifier })),
    [{ owner: "src/producer.ts", specifier: "./support.svg" }],
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
  assert.throws(() => session.moduleCss("src/producer.ts"), /Unknown Stillwind module/);
  session.compile(
    sources.map((source) => ({
      ...source,
      source:
        source.path === "src/producer.ts"
          ? source.source.replaceAll("opacity-0", "opacity-5")
          : source.source,
      type_facts: facts.get(source.path) ?? [],
    })),
    "src/consumer.ts",
  );
  const changedPlan = session.planStylesheet(selection) as typeof plan;
  assert.notEqual(changedPlan.producers[0].semanticRevision, plan.producers[0].semanticRevision);
  // CSS values may retain calc() until the bundler's optimization stage.
  assert.match(foldCalcExpressions(changedPlan.css), /opacity:5%/);

  const runnable = finalizeReactModule(consumer.code, false);
  assert.doesNotMatch(runnable, /from "stillwind\/react"/);
  assert.match(
    runnable,
    /^import \{ stillwindAssetClass, stillwindClassKey, stillwindHandleClass, stillwindMerge, stillwindStyleKey, stillwindViewTransitionStyle \} from "stillwind\/react\/runtime";/,
  );
  for (const helper of runnable.matchAll(/\b(stillwind[A-Z]\w*)\s*\(/g)) {
    assert.match(runnable, new RegExp(`import \\{[^}]*\\b${helper[1]}\\b[^}]*\\}`));
  }
});
