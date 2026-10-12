import path from "node:path";

import { scanSource, type NativeProjectSession } from "@stillwind/native";
import { expect, test } from "vite-plus/test";

import type { TypeScriptFactsService } from "../semantics/type-facts.ts";
import { compileModule } from "./compiler.ts";
import { fileModuleDescriptor } from "./filesystem.ts";

test("a rejected generator name is reported through the host warnings", async () => {
  // A rejected name comes from the host's generator rather than from the source, so it has no
  // position to report as a diagnostic or a lint finding.
  const session = {
    scanSource,
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
    {
      resolve: async () => null,
      read: async () => {
        expect.fail("unexpected read");
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
  expect(output?.code).toBeTruthy();
  expect(warnings.length).toBe(1);
  expect(warnings[0]!).toMatch(/class/);
  expect(warnings[0]!).toMatch(/the host generator failed/);
  expect(warnings[0]!).toMatch(/a{64}/);
});
