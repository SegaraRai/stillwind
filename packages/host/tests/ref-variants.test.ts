import path from "node:path";
import { fileURLToPath } from "node:url";

import { compileProject, summarizeSource } from "@stillwind/native";
import { expect, test } from "vite-plus/test";

import type { ModuleSummary } from "../src/protocol.ts";
import { TypeScriptFactsService } from "../src/type-facts.ts";

const cssInput = (content: string) => ({
  content,
  name: "root.css",
  uri: "untitled:root.css",
  sourceMapSource: "untitled:root.css",
});

test("direct ref variants preserve imported conditional ref/boundary pairs", () => {
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
        moduleId: source.moduleId,
        kind: "direct" as const,
        document: {
          uri: `untitled:${source.moduleId}`,
          code: source.source,
          format: "script" as const,
          sourceMapSource: `untitled:${source.moduleId}`,
        },
        syntax: {
          language: "ts" as const,
          moduleKind: "unambiguous" as const,
        },
        resolverId: path.join(root, source.moduleId),
        checkerFileName: path.join(root, source.moduleId),
        projectionFileName: path.join(root, source.moduleId),
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
      expect(output).toBeTruthy();
      expect(output!.diagnostics).toStrictEqual([]);
      if (expression.includes(".at")) {
        expect(new Set(output!.css.match(/@container sw-c-[\w-]+/g)).size).toBe(2);
        expect(output!.css).toMatch(/\(width > 40rem\)/);
        expect(output!.css).not.toMatch(/:has\(/);
      } else {
        // The pairs are read from the compiled selectors: each boundary must bind to
        // exactly one ref of the pair.
        const pairs = [
          ...output!.css.matchAll(/:where\(\.(sw_\w+)\):has\(:where\(\.(sw_\w+)\):hover\)/g),
        ].map((match) => ({ boundary: match[1], ref: match[2] }));
        expect(pairs.length, output!.css).toBe(2);
        const [first, second] = pairs;
        expect(first.boundary).not.toBe(second.boundary);
        expect(first.ref).not.toBe(second.ref);
        expect(output!.css, output!.css).toContain(
          `:where(.${first.boundary}):has(:where(.${first.ref}):hover)`,
        );
        expect(output!.css, output!.css).toContain(
          `:where(.${second.boundary}):has(:where(.${second.ref}):hover)`,
        );
        expect(output!.css, output!.css).not.toContain(
          `:where(.${first.boundary}):has(:where(.${second.ref}):`,
        );
        expect(output!.css, output!.css).not.toContain(
          `:where(.${second.boundary}):has(:where(.${first.ref}):`,
        );
      }
      expect(output!.css).toMatch(/:active\{color:red;/);
      expect(output!.code).not.toMatch(/selected\(|selected`|selected\.at/);
      outputs.push(output);
    }
    expect(outputs[0]!.css).toBe(outputs[1]!.css);
    expect(outputs[0]!.code).toBe(outputs[1]!.code);
    expect(outputs[2]!.css).toBe(outputs[3]!.css);
    expect(outputs[2]!.code).toBe(outputs[3]!.code);
  } finally {
    service.close();
  }
});
