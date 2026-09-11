import assert from "node:assert/strict";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { compileReactProject, summarizeReact } from "@stillwind/native";

import type { ModuleSummary } from "../src/protocol.ts";
import { TypeScriptFactsService } from "../src/type-facts.ts";

void test("direct ref variants preserve imported conditional ref/scope pairs", () => {
  const root = path.dirname(fileURLToPath(import.meta.url));
  const producer = `import {style} from 'stillwind/react';
export const scopeA = style.scope('scopeA');
export const scopeB = style.scope('scopeB');
export const a = style.ref(scopeA, 'a');
export const b = style.ref(scopeB, 'b');`;
  const service = new TypeScriptFactsService(root);
  try {
    const outputs = [];
    for (const expression of [
      "`${selected`hover:`}:active:accent`",
      '{ [selected("hover")]: { "active:": "accent" } }',
    ]) {
      const consumer = `import {style} from 'stillwind/react';
import {a, b} from './ref-producer';
declare const enabled: boolean;
const selected = enabled ? a : b;
export const props = style(${expression});`;
      const sources = [
        { path: "ref-producer.ts", source: producer },
        { path: "ref-consumer.ts", source: consumer },
      ].map((source) => ({
        ...source,
        id: path.join(root, source.path),
        summary: summarizeReact(source.source, source.path) as ModuleSummary,
      }));
      const facts = service.collect(
        sources.map((source) => ({ ...source, probes: source.summary.type_probes })),
        new Map(sources.map((source) => [source.path, source.summary.assets])),
      );
      const result = compileReactProject(
        sources.map((source) => ({ ...source, type_facts: facts.get(source.path) ?? [] })),
        `@variant hover { @media (hover:hover) { &:hover { @slot; } } }
@variant active { &:active { @slot; } }
@utility accent { color:red; }
@stillwind-slot utilities;`,
      ) as {
        modules: Array<{ path: string; code: string; css: string; diagnostics: unknown[] }>;
      };
      const output = result.modules.find((module) => module.path === "ref-consumer.ts");
      assert.ok(output);
      assert.deepEqual(output.diagnostics, []);
      assert.match(output.css, /\.s_scopeA_\w+:has\(\.s_a_\w+:where\(:hover\)\)/);
      assert.match(output.css, /\.s_scopeB_\w+:has\(\.s_b_\w+:where\(:hover\)\)/);
      assert.doesNotMatch(output.css, /\.s_scopeA_\w+:has\(\.s_b_/);
      assert.doesNotMatch(output.css, /\.s_scopeB_\w+:has\(\.s_a_/);
      assert.match(output.css, /:active\{color:red;/);
      assert.doesNotMatch(output.code, /selected\(|selected`/);
      outputs.push(output);
    }
    assert.equal(outputs[0].css, outputs[1].css);
    assert.equal(outputs[0].code, outputs[1].code);
  } finally {
    service.close();
  }
});
