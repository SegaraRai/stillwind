import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { compileReactProject, summarizeReact } from "@stillwind/native";
import { finalizeReactModule } from "../src/index.ts";
import { TypeScriptFactsService, type AssetRecord } from "../src/type-facts.ts";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../..");
const fixtureRoot = path.join(path.dirname(fileURLToPath(import.meta.url)), "fixtures/type-facts");

const sources = ["producer.ts", "chooser.ts", "barrel.ts", "consumer.ts"].map((name) => ({
  id: path.join(fixtureRoot, name),
  path: `src/${name}`,
  source: readFileSync(path.join(fixtureRoot, name), "utf8"),
}));

const summaries = new Map(
  sources.map((source) => [
    source.path,
    JSON.parse(summarizeReact(source.source, source.path)) as { assets: AssetRecord[] },
  ]),
);

void test("TypeScript is required for cross-module conditional handle propagation", () => {
  const withoutFacts = JSON.parse(
    compileReactProject(
      JSON.stringify(sources.map(({ path: sourcePath, source }) => ({ path: sourcePath, source }))),
      "@theme {}",
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
  assert.deepEqual(service.stats, {
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
      "@theme {}",
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
