import assert from "node:assert/strict";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { summarizeSource } from "@stillwind/native";

import type { ModuleSummary } from "../src/protocol.ts";
import { TypeScriptFactsService } from "../src/type-facts.ts";

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
        [{ id, path: "probe-document.ts", source, probes: summary.type_probes }],
        new Map([["probe-document.ts", summary.assets]]),
      )
      .get("probe-document.ts")!;
    assert.ok(facts.length > 0);
    for (const fact of facts) {
      assert.ok(
        summary.type_probes.some((probe) => probe.start === fact.start && probe.end === fact.end),
      );
    }
    assert.equal(service.stats.restarts, 0);
    const noProbes = service.collect(
      [{ id, path: "probe-document.ts", source, probes: [] }],
      new Map([["probe-document.ts", summary.assets]]),
    );
    assert.deepEqual(noProbes.get("probe-document.ts"), []);
  } finally {
    service.close();
  }
});
