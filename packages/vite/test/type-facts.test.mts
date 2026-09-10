import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { TypeScriptFactsService, type ModuleSummary } from "@stillwind/host";
import {
  compileReactProject,
  summarizeReact,
  NativeDesignSystemLoader,
  NativeProjectSession,
} from "@stillwind/native";

import { resolveFixtureCss } from "../../../tools/lib/stillwind-fixture-css.ts";
import { finalizeReactModule } from "../src/index.ts";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../..");
const fixtureRoot = path.join(path.dirname(fileURLToPath(import.meta.url)), "fixtures/type-facts");

const sources = ["producer.ts", "chooser.ts", "barrel.ts", "consumer.ts"]
  .map((name) => ({
    id: path.join(fixtureRoot, name),
    path: `src/${name}`,
    source: readFileSync(path.join(fixtureRoot, name), "utf8"),
  }))
  .map((source) => ({
    ...source,
    probes: (JSON.parse(summarizeReact(source.source, source.path)) as ModuleSummary).type_probes,
  }));

const summaries = new Map(
  sources.map((source) => [
    source.path,
    JSON.parse(summarizeReact(source.source, source.path)) as ModuleSummary,
  ]),
);

void test("TypeScript is required for cross-module conditional handle propagation", () => {
  const withoutFacts = JSON.parse(
    compileReactProject(
      JSON.stringify(sources.map(({ path: sourcePath, source }) => ({ path: sourcePath, source }))),
      resolveFixtureCss('@import "stillwind";'),
    ),
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
  const withFacts = JSON.parse(
    compileReactProject(
      JSON.stringify(
        sources.map(({ path: sourcePath, source }) => ({
          path: sourcePath,
          source,
          type_facts: facts.get(sourcePath) ?? [],
        })),
      ),
      resolveFixtureCss('@import "stillwind";'),
    ),
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
  const selected = JSON.parse(
    session.compile(
      JSON.stringify(
        sources.map((source) => ({
          ...source,
          type_facts: facts.get(source.path) ?? [],
        })),
      ),
      "src/consumer.ts",
    ),
  ) as typeof withFacts;
  assert.deepEqual(
    selected.modules.map((module) => module.path),
    ["src/consumer.ts"],
  );
  assert.deepEqual(selected.modules[0].diagnostics, []);
  assert.match(session.moduleCss("src/consumer.ts"), /animation-name:--sw-a-/);
  const selection = {
    modules: ["src/consumer.ts"],
    coverage: { kind: "complete", target: "consumer-route", revision: "graph-1" },
    placement: "carrier",
  };
  const plan = JSON.parse(session.planStylesheet(JSON.stringify(selection))) as {
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
      session.planStylesheet(
        JSON.stringify({
          ...selection,
          modules: [...selection.modules, "src/unvisited.ts"],
        }),
      ),
    /unregistered modules/,
  );
  assert.throws(() => session.moduleCss("src/producer.ts"), /Unknown Stillwind module/);
  session.compile(
    JSON.stringify(
      sources.map((source) => ({
        ...source,
        source:
          source.path === "src/producer.ts"
            ? source.source.replaceAll("opacity-0", "opacity-5")
            : source.source,
        type_facts: facts.get(source.path) ?? [],
      })),
    ),
    "src/consumer.ts",
  );
  const changedPlan = JSON.parse(session.planStylesheet(JSON.stringify(selection))) as typeof plan;
  assert.notEqual(changedPlan.producers[0].semanticRevision, plan.producers[0].semanticRevision);
  assert.match(changedPlan.css, /opacity:5%/);

  const runnable = finalizeReactModule(consumer.code, false);
  assert.doesNotMatch(runnable, /from "stillwind\/react"/);
  assert.match(
    runnable,
    /^import \{ stillwindAssetClass, stillwindHandleClass, stillwindViewTransitionStyle \} from "stillwind\/react\/runtime";/,
  );
  for (const helper of runnable.matchAll(/\b(stillwind[A-Z]\w*)\s*\(/g)) {
    assert.match(runnable, new RegExp(`import \\{[^}]*\\b${helper[1]}\\b[^}]*\\}`));
  }
});
