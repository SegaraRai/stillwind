import path from "node:path";

import { NativeDesignSystemLoader, NativeProjectSession } from "@stillwind/native";
import { expect, test } from "vite-plus/test";

import { compileModule, fileModuleDescriptor } from "../src/compiler.ts";
import { TypeScriptFactsService } from "../src/type-facts.ts";

test("contract defaults deny builds while warn and off remain configurable", async () => {
  for (const level of [undefined, "warn", "off"] as const) {
    const loader = new NativeDesignSystemLoader();
    loader.provide(
      0,
      "theme.css",
      `${level ? `@stillwind {macro-argument-mutation: ${level};}` : ""} @stillwind-slot utilities;`,
    );
    const session = new NativeProjectSession(loader, false);
    const warnings: string[] = [];
    const facts = new TypeScriptFactsService(process.cwd());
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
      if (level === undefined) {
        await expect(promise).rejects.toThrow(/macro-argument-mutation/);
      } else {
        expect((await promise)?.code).toBeTruthy();
        expect(warnings.length).toBe(level === "warn" ? 1 : 0);
      }
    } finally {
      facts.close();
    }
  }
});
