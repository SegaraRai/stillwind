import assert from "node:assert/strict";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { compileProject, summarizeSource } from "@stillwind/native";

import type { ModuleSummary } from "../src/protocol.ts";
import { TypeScriptFactsService } from "../src/type-facts.ts";

void test("checker identities reject unconsumed imported, aliased and finite dynamic uses", () => {
  const root = path.dirname(fileURLToPath(import.meta.url));
  const producer = `import {style} from 'stillwind/react';
export const a=style.ref(); export const b=style.ref();
export const animation=style.keyframes({from:'opacity-0'});
export const transition=style.viewTransition({old:'opacity-0'});`;
  const service = new TypeScriptFactsService(root);
  try {
    for (const expression of [
      "alias.asAnchor()",
      "alias.asContainer()",
      "alias.positionAnchor",
      "alias()",
      "alias('hover:')",
      "alias`hover:`",
      "alias.at('width > 1px')",
      "alias.at`width > 1px`",
      "selected.asAnchor()",
      "selected.positionAnchor",
      "selected('hover:')",
      "selected.at`width > 1px`",
      "motion.for('hero')",
      "motion.for(key)",
      "frames('1s')",
    ]) {
      const consumer = `import {style} from 'stillwind/react';
import {a,b,animation,transition} from './consumption-producer';
const alias=a; const frames=animation; const motion=transition;
declare const enabled:boolean; declare const key:string;
const selected=enabled ? a : b;
export const invalid=${expression};`;
      const sources = [
        { module_id: "consumption-producer.ts", source: producer },
        { module_id: "consumption-consumer.ts", source: consumer },
      ].map((entry) => ({
        ...entry,
        uri: `untitled:${entry.module_id}`,
        source_map_source: `untitled:${entry.module_id}`,
        language: "ts" as const,
        module_kind: "unambiguous" as const,
        resolver_id: path.join(root, entry.module_id),
        checker_file_name: path.join(root, entry.module_id),
        summary: summarizeSource(
          entry.source,
          entry.module_id,
          "ts",
          "unambiguous",
        ) as ModuleSummary,
      }));
      const facts = service.collect(
        sources.map((entry) => ({ ...entry, probes: entry.summary.type_probes })),
        new Map(sources.map((entry) => [entry.module_id, entry.summary.assets])),
      );
      const result = compileProject(
        sources.map((entry) => ({ ...entry, type_facts: facts.get(entry.module_id) ?? [] })),
        {
          content: "@utility opacity-0 { opacity:0; } @stillwind-slot utilities;",
          name: "root.css",
          uri: "untitled:root.css",
          source_map_source: "untitled:root.css",
        },
      );
      const output = result.modules.find((entry) => entry.module_id === "consumption-consumer.ts");
      assert.ok(output);
      const code = expression.startsWith("frames(") ? "SHANIMATION002" : "SHMACRO003";
      assert.ok(
        output.diagnostics.some((error) => error.code === code),
        `${expression}: ${JSON.stringify(output.diagnostics)}`,
      );
    }
  } finally {
    service.close();
  }
});
