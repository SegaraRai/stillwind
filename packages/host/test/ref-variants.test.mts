import assert from "node:assert/strict";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { compileProject, summarizeSource } from "@stillwind/native";

import type { ModuleSummary } from "../src/protocol.ts";
import { TypeScriptFactsService } from "../src/type-facts.ts";

const cssInput = (content: string) => ({
  content,
  name: "root.css",
  uri: "untitled:root.css",
  sourceMapSource: "untitled:root.css",
});

void test("direct ref variants preserve imported conditional ref/boundary pairs", () => {
  const root = path.dirname(fileURLToPath(import.meta.url));
  const producer = `import {style} from 'stillwind/react';
export const boundaryA = style.ref('boundaryA');
export const boundaryB = style.ref('boundaryB');
export const a = style.ref(boundaryA, 'a');
export const b = style.ref(boundaryB, 'b');`;
  const service = new TypeScriptFactsService(root);
  try {
    const outputs = [];
    for (const expression of [
      "`${selected`hover:`}:active:accent`",
      '{ [selected("hover")]: { "active:": "accent" } }',
      '{ [selected.at("width > 40rem")]: { "active:": "accent" } }',
      "`${selected.at`width > 40rem`}:active:accent`",
    ]) {
      const consumer = `import {style} from 'stillwind/react';
import {a, b} from './ref-producer';
declare const enabled: boolean;
const selected = enabled ? a : b;
export const props = style(${expression});`;
      const sources = [
        { moduleId: "ref-producer.ts", uri: "untitled:ref-producer.ts", source: producer },
        { moduleId: "ref-consumer.ts", uri: "untitled:ref-consumer.ts", source: consumer },
      ].map((source) => ({
        ...source,
        language: "ts" as const,
        moduleKind: "unambiguous" as const,
        sourceMapSource: source.uri,
        resolverId: path.join(root, source.moduleId),
        checkerFileName: path.join(root, source.moduleId),
        summary: summarizeSource(
          source.source,
          source.moduleId,
          "tsx",
          "unambiguous",
        ) as ModuleSummary,
      }));
      const facts = service.collect(
        sources.map((source) => ({ ...source, probes: source.summary.typeProbes })),
        new Map(sources.map((source) => [source.moduleId, source.summary.assets])),
      );
      const result = compileProject(
        sources.map((source) => ({ ...source, typeFacts: facts.get(source.moduleId) ?? [] })),
        cssInput(`@variant hover { @media (hover:hover) { &:hover { @slot; } } }
@variant active { &:active { @slot; } }
@utility accent { color:red; }
@stillwind-slot utilities;`),
      ) as {
        modules: Array<{ moduleId: string; code: string; css: string; diagnostics: unknown[] }>;
      };
      const output = result.modules.find((module) => module.moduleId === "ref-consumer.ts");
      assert.ok(output);
      assert.deepEqual(output.diagnostics, []);
      if (expression.includes(".at")) {
        assert.equal(new Set(output.css.match(/@container sw-c-[\w-]+/g)).size, 2);
        assert.match(output.css, /\(width > 40rem\)/);
        assert.doesNotMatch(output.css, /:has\(/);
      } else {
        // The pairs are read from the compiled selectors: each boundary must bind to
        // exactly one ref of the pair.
        const pairs = [
          ...output.css.matchAll(/:where\(\.(sw_\w+)\):has\(:where\(\.(sw_\w+)\):hover\)/g),
        ].map((match) => ({ boundary: match[1], ref: match[2] }));
        assert.equal(pairs.length, 2, output.css);
        const [first, second] = pairs;
        assert.notEqual(first.boundary, second.boundary);
        assert.notEqual(first.ref, second.ref);
        assert.ok(
          output.css.includes(`:where(.${first.boundary}):has(:where(.${first.ref}):hover)`),
          output.css,
        );
        assert.ok(
          output.css.includes(`:where(.${second.boundary}):has(:where(.${second.ref}):hover)`),
          output.css,
        );
        assert.ok(
          !output.css.includes(`:where(.${first.boundary}):has(:where(.${second.ref}):`),
          output.css,
        );
        assert.ok(
          !output.css.includes(`:where(.${second.boundary}):has(:where(.${first.ref}):`),
          output.css,
        );
      }
      assert.match(output.css, /:active\{color:red;/);
      assert.doesNotMatch(output.code, /selected\(|selected`|selected\.at/);
      outputs.push(output);
    }
    assert.equal(outputs[0].css, outputs[1].css);
    assert.equal(outputs[0].code, outputs[1].code);
    assert.equal(outputs[2].css, outputs[3].css);
    assert.equal(outputs[2].code, outputs[3].code);
  } finally {
    service.close();
  }
});
