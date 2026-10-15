import path from "node:path";

import { NativeDesignSystemLoader, NativeProjectSession } from "@stillwind/native";
import { expect, test } from "vite-plus/test";

import { compileModule, fileModuleDescriptor } from "../src/compilation/index.ts";
import { TypeScriptFactsService } from "../src/semantics/index.ts";

test("macro contract lint retains its severity without rejecting compilation", async () => {
  for (const level of [undefined, "warn", "off", "deny"] as const) {
    const loader = new NativeDesignSystemLoader({
      lint: {
        mutatingMacroArgument: level,
      },
    });
    loader.provide(0, "theme.css", "@stillwind-slot utilities;");
    const session = new NativeProjectSession(loader, false);
    const warnings: string[] = [];
    const facts = new TypeScriptFactsService({
      root: process.cwd(),
      project: {
        kind: "isolated",
      },
    });
    const promise = compileModule(
      session,
      () => facts,
      "import {style} from 'stillwind/react'; let n = 0; export const props = style({'--x': n++});",
      path.resolve("contract.tsx"),
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
    try {
      const output = await promise;
      expect(output?.code).toBeTruthy();
      expect(warnings.length).toBe(level === "off" ? 0 : 1);
      if (level !== "off") {
        expect(output?.reports).toContainEqual(
          expect.objectContaining({
            code: "mutating-macro-argument",
            severity: level === "warn" ? "warning" : "error",
          }),
        );
      }
    } finally {
      facts.close();
    }
  }
});
