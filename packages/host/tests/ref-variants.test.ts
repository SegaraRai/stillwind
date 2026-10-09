import path from "node:path";
import { fileURLToPath } from "node:url";

import { compileProject, summarizeSource, type NativeModuleSummary } from "@stillwind/native";
import { expect, test } from "vite-plus/test";

import { TypeScriptFactsService } from "../src/semantics/index.ts";

const cssInput = (content: string) => ({
  content,
  name: "root.css",
  uri: "untitled:root.css",
  sourceMapSource: "untitled:root.css",
});

test("direct ref variants preserve imported conditional refs with use-site scopes", () => {
  const root = path.dirname(fileURLToPath(import.meta.url));
  const producer = `import {style} from 'stillwind/react';
export const scope = style.ref('scope');
export const a = style.ref('a');
export const b = style.ref('b');`;
  const service = new TypeScriptFactsService({
    root,
    project: {
      kind: "isolated",
    },
  });
  try {
    const outputs = [];
    for (const expression of [
      "`${selected`hover:`.in(scope)}:active:accent`",
      '{ [selected("hover").in(scope)]: { "active:": "accent" } }',
      '{ [selected.at("width > 40rem")]: { "active:": "accent" } }',
      "`${selected.at`width > 40rem`}:active:accent`",
    ]) {
      const consumer = `import {style} from 'stillwind/react';
import {a, b, scope} from './ref-producer';
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
        ) as NativeModuleSummary,
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
        modules: {
          moduleId: string;
          code: string;
          css: string;
          diagnostics: unknown[];
        }[];
      };
      const output = result.modules.find((module) => module.moduleId === "ref-consumer.ts");
      expect(output).toBeTruthy();
      expect(output!.diagnostics).toStrictEqual([]);
      if (expression.includes(".at")) {
        expect(new Set(output!.css.match(/@container sw-c-[\w-]+/g)).size).toBe(2);
        expect(output!.css).toMatch(/\(width > 40rem\)/);
        expect(output!.css).not.toMatch(/:has\(/);
      } else {
        const pairs = [
          ...output!.css.matchAll(
            /:is\(:where\(\.(sw_\w+)\)\):has\(:is\(:where\(\.(sw_\w+)\):hover\)\)/g,
          ),
        ].map((match) => ({ scope: match[1], ref: match[2] }));
        expect(pairs, output!.css).toHaveLength(2);
        expect(new Set(pairs.map((pair) => pair.scope)).size).toBe(1);
        expect(new Set(pairs.map((pair) => pair.ref)).size).toBe(2);
      }
      expect(output!.css).toMatch(/\.sw_\w+:active(?::is\([^{}]*\))?\{color:red;/);
      expect(output!.code).not.toMatch(/selected\(|selected`|selected\.at/);
      outputs.push(output);
    }
    const normalizeTargetClasses = (text: string) => text.replace(/\bsw_[a-z0-9]+\b/g, "sw_target");
    for (const [first, second] of [
      [outputs[0]!, outputs[1]!],
      [outputs[2]!, outputs[3]!],
    ]) {
      expect(normalizeTargetClasses(first.css)).toBe(normalizeTargetClasses(second.css));
      expect(normalizeTargetClasses(first.code)).toBe(normalizeTargetClasses(second.code));
    }
  } finally {
    service.close();
  }
});
