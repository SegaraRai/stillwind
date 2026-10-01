import assert from "node:assert/strict";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { summarizeSource } from "@stillwind/native";

import { CheckerSource, identityKey, type TypeFactSource } from "../src/checker-source.ts";
import type { ModuleSummary } from "../src/protocol.ts";
import { TypeScriptFactsService } from "../src/type-facts.ts";

for (const [kind, language] of [
  ["direct", "js"],
  ["projected", "js"],
  ["projected", "ts"],
] as const) {
  void test(`${kind} ${language} checker shadows use identity syntax accepted by their language`, () => {
    const code = 'import { style } from "stillwind/react"; const value = style("p-2");';
    const summary = summarizeSource(code, "shadow", language, "module") as ModuleSummary;
    assert.equal(summary.assets.length, 1);
    const prefix = kind === "projected" ? "😀<script>" : "";
    const byteOffset = Buffer.byteLength(prefix);
    const assets = summary.assets.map((asset) => ({
      ...asset,
      callStart: asset.callStart + byteOffset,
      callEnd: asset.callEnd + byteOffset,
      identity: {
        ...asset.identity,
        start: asset.identity.start + byteOffset,
        end: asset.identity.end + byteOffset,
      },
    }));
    const common = {
      resolverId: "shadow",
      checkerFileName: "shadow.js",
      projectionFileName: "shadow.vue",
      moduleId: "shadow",
      probes: [],
    };
    const syntax = { language, moduleKind: "module" as const };
    const source: TypeFactSource =
      kind === "direct"
        ? {
            ...common,
            kind,
            document: {
              uri: "test:///shadow.js",
              code,
              format: "script",
            },
            syntax,
          }
        : {
            ...common,
            kind,
            document: {
              uri: "test:///shadow.vue",
              code: `${prefix}${code}</script>`,
              format: "vue",
            },
            projection: {
              code,
              syntax,
              map: {
                kind: "volar",
                mappings: [
                  {
                    sourceOffsets: [prefix.length],
                    generatedOffsets: [0],
                    lengths: [code.length],
                  },
                ],
              },
              converter: {
                name: "test",
                version: "1",
                optionsFingerprint: "test",
              },
              importRealms: [],
              editableRegions: [],
            },
            checkerSpans: assets.map((asset) => ({
              start: asset.callStart,
              end: asset.callEnd,
              projectedStart: asset.callStart - byteOffset,
              projectedEnd: asset.callEnd - byteOffset,
            })),
          };
    const shadow = new CheckerSource(
      source,
      { source: code, language: "ts" },
      0,
      assets,
      new Map(),
    );
    assert.ok(shadow.text.includes(identityKey(assets[0].identity)));
    if (language === "js") {
      assert.match(shadow.text, /\/\*\* @type \{\{readonly __stillwindDeclarationIdentity:/);
      assert.doesNotMatch(shadow.text, /\bas unknown as\b/);
    } else {
      assert.match(
        shadow.text,
        /undefined as unknown as \{readonly __stillwindDeclarationIdentity:/,
      );
      assert.doesNotMatch(shadow.text, /@type/);
    }
  });
}

void test("only requested UTF-8 probe spans are queried beside unrelated generic tuples", () => {
  const root = path.dirname(fileURLToPath(import.meta.url));
  const id = path.join(root, "probe-document.ts");
  const source = `import {style} from 'stillwind/react';
// 非ASCII 🧭 offsets must survive shadow identity insertion.
declare function tuple<T>(value: T): [T, T];
const unrelated = tuple(123);
const token = style.keyframes({from: 'opacity-0', to: 'opacity-100'});
const selected = token;
export const props = style(selected('infinite'));`;
  const summary = summarizeSource(
    source,
    "probe-document.ts",
    "ts",
    "unambiguous",
  ) as ModuleSummary;
  const service = new TypeScriptFactsService(root);
  try {
    const facts = service
      .collect(
        [
          {
            resolverId: id,
            checkerFileName: id,
            moduleId: "probe-document.ts",
            projectionFileName: id,
            kind: "direct",
            document: {
              uri: `untitled:${id}`,
              code: source,
              format: "script",
            },
            syntax: {
              language: "ts",
              moduleKind: "unambiguous",
            },
            probes: summary.typeProbes,
          },
        ],
        new Map([["probe-document.ts", summary.assets]]),
      )
      .get("probe-document.ts")!;
    assert.ok(facts.length > 0);
    for (const fact of facts) {
      assert.ok(
        summary.typeProbes.some((probe) => probe.start === fact.start && probe.end === fact.end),
      );
    }
    assert.equal(service.stats.restarts, 0);
    const noProbes = service.collect(
      [
        {
          resolverId: id,
          checkerFileName: id,
          moduleId: "probe-document.ts",
          projectionFileName: id,
          kind: "direct",
          document: {
            uri: `untitled:${id}`,
            code: source,
            format: "script",
          },
          syntax: {
            language: "ts",
            moduleKind: "unambiguous",
          },
          probes: [],
        },
      ],
      new Map([["probe-document.ts", summary.assets]]),
    );
    assert.deepEqual(noProbes.get("probe-document.ts"), []);
  } finally {
    service.close();
  }
});
