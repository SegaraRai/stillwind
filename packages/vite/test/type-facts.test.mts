import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { compileReactProject, summarizeReact } from "@stillwind/native";
import { collectTypeScriptFacts, type AssetRecord } from "../src/type-facts.ts";

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

  const facts = collectTypeScriptFacts(
    sources,
    new Map([...summaries].map(([sourcePath, summary]) => [sourcePath, summary.assets])),
    repositoryRoot,
  );
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
  assert.equal(consumer.css.match(/anchor-name:--sw-r-/g)?.length, 2);
});
