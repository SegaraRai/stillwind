import assert from "node:assert/strict";
import path from "node:path";
import test from "node:test";

import type { NativeProjectSession } from "@stillwind/native";

import { compileModule, fileModuleDescriptor } from "../src/compiler.ts";
import type { TypeScriptFactsService } from "../src/type-facts.ts";

void test("a rejected generator name is reported through the host warnings", async () => {
  // A rejected name comes from the host's generator rather than from the source, so it has no
  // position to report as a diagnostic or a lint finding.
  const session = {
    compile: () => ({
      modules: [
        {
          moduleId: "rejected.tsx",
          code: "export const x = undefined;",
          diagnostics: [],
          lintFindings: [],
          lint_policy: {},
          composition: [],
          rejectedNames: [
            {
              kind: "class",
              hash: "a".repeat(64),
              reason: "the host generator failed",
              substituted: true,
            },
          ],
        },
      ],
    }),
  } as unknown as NativeProjectSession;
  const warnings: string[] = [];
  const output = await compileModule(
    session,
    () => ({}) as TypeScriptFactsService,
    "export const x = 1;",
    path.resolve("rejected.tsx"),
    process.cwd(),
    {
      resolve: async () => null,
      read: async () => {
        throw new Error("unexpected read");
      },
      describe: (id) => ({
        ...fileModuleDescriptor(process.cwd(), id),
        document: {
          ...fileModuleDescriptor(process.cwd(), id).document,
          uri: `untitled:${encodeURIComponent(id)}`,
        },
      }),
      watch: () => {},
      warn: (message) => warnings.push(message),
    },
  );
  assert.ok(output?.code);
  assert.equal(warnings.length, 1);
  assert.match(warnings[0]!, /class/);
  assert.match(warnings[0]!, /the host generator failed/);
  assert.match(warnings[0]!, /a{64}/);
});
