import path from "node:path";
import { fileURLToPath } from "node:url";

import { compileProject, summarizeSource, type NativeModuleSummary } from "@stillwind/native";
import { expect, test } from "vite-plus/test";

import { TypeScriptFactsService } from "../src/semantics/index.ts";

test("checker identities reject unconsumed imported, aliased and finite dynamic uses", () => {
  const root = path.dirname(fileURLToPath(import.meta.url));
  const producer = `import {style} from 'stillwind/react';
export const a=style.ref(); export const b=style.ref();
export const animation=style.keyframes({from:'opacity-0'});
export const transition=style.viewTransition({old:'opacity-0'});`;
  const service = new TypeScriptFactsService({
    root,
    project: {
      kind: "isolated",
    },
  });
  try {
    for (const expression of [
      "alias.asAnchor()",
      "alias.asContainer()",
      "alias.positionAnchor",
      "alias.positionAnchor()",
      "alias()",
      "alias('hover:')",
      "alias`hover:`",
      "alias.at('width > 1px')",
      "alias.at`width > 1px`",
      "selected.asAnchor()",
      "selected.positionAnchor",
      "selected.positionAnchor()",
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
        { moduleId: "consumption-producer.ts", source: producer },
        { moduleId: "consumption-consumer.ts", source: consumer },
      ].map((entry) => ({
        moduleId: entry.moduleId,
        kind: "direct" as const,
        document: {
          uri: `untitled:${entry.moduleId}`,
          code: entry.source,
          format: "script" as const,
          sourceMapSource: `untitled:${entry.moduleId}`,
        },
        syntax: {
          language: "ts" as const,
          moduleKind: "unambiguous" as const,
        },
        resolverId: path.join(root, entry.moduleId),
        checkerFileName: path.join(root, entry.moduleId),
        projectionFileName: path.join(root, entry.moduleId),
        summary: summarizeSource(
          entry.source,
          entry.moduleId,
          "ts",
          "unambiguous",
        ) as NativeModuleSummary,
      }));
      const facts = service.collect(
        sources.map((entry) => ({ ...entry, probes: entry.summary.typeProbes })),
        new Map(sources.map((entry) => [entry.moduleId, entry.summary.assets])),
      );
      const result = compileProject(
        sources.map((entry) => ({ ...entry, typeFacts: facts.get(entry.moduleId) ?? [] })),
        {
          content: "@utility opacity-0 { opacity:0; } @stillwind-slot utilities;",
          name: "root.css",
          uri: "untitled:root.css",
          sourceMapSource: "untitled:root.css",
        },
      );
      const output = result.modules.find((entry) => entry.moduleId === "consumption-consumer.ts");
      expect(output).toBeTruthy();
      const code = expression.startsWith("frames(") ? "SW04002" : "SW01011";
      expect(
        output!.diagnostics.some((error) => error.code === code),
        `${expression}: ${JSON.stringify(output!.diagnostics)}`,
      ).toBeTruthy();
    }
  } finally {
    service.close();
  }
});
