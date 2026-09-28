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
  source_map_source: "untitled:root.css",
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
        { module_id: "ref-producer.ts", uri: "untitled:ref-producer.ts", source: producer },
        { module_id: "ref-consumer.ts", uri: "untitled:ref-consumer.ts", source: consumer },
      ].map((source) => ({
        ...source,
        language: "ts" as const,
        module_kind: "unambiguous" as const,
        source_map_source: source.uri,
        resolver_id: path.join(root, source.module_id),
        checker_file_name: path.join(root, source.module_id),
        summary: summarizeSource(
          source.source,
          source.module_id,
          "tsx",
          "unambiguous",
        ) as ModuleSummary,
      }));
      const facts = service.collect(
        sources.map((source) => ({ ...source, probes: source.summary.type_probes })),
        new Map(sources.map((source) => [source.module_id, source.summary.assets])),
      );
      const result = compileProject(
        sources.map((source) => ({ ...source, type_facts: facts.get(source.module_id) ?? [] })),
        cssInput(`@variant hover { @media (hover:hover) { &:hover { @slot; } } }
@variant active { &:active { @slot; } }
@utility accent { color:red; }
@stillwind-slot utilities;`),
      ) as {
        modules: Array<{ module_id: string; code: string; css: string; diagnostics: unknown[] }>;
      };
      const output = result.modules.find((module) => module.module_id === "ref-consumer.ts");
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
          ...output.css.matchAll(
            /@media \(hover:hover\)\{\.(sw_\w+):has\(\.(sw_\w+):where\(:hover\)\)/g,
          ),
        ].map((match) => ({ boundary: match[1], ref: match[2] }));
        assert.equal(pairs.length, 2, output.css);
        const [first, second] = pairs;
        assert.notEqual(first.boundary, second.boundary);
        assert.notEqual(first.ref, second.ref);
        assert.ok(
          output.css.includes(`.${first.boundary}:has(.${first.ref}:where(:hover))`),
          output.css,
        );
        assert.ok(
          output.css.includes(`.${second.boundary}:has(.${second.ref}:where(:hover))`),
          output.css,
        );
        assert.ok(!output.css.includes(`.${first.boundary}:has(.${second.ref}:`), output.css);
        assert.ok(!output.css.includes(`.${second.boundary}:has(.${first.ref}:`), output.css);
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
