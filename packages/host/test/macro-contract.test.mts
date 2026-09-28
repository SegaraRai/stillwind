import assert from "node:assert/strict";
import path from "node:path";
import test from "node:test";

import { NativeDesignSystemLoader, NativeProjectSession } from "@stillwind/native";

import { compileModule, fileModuleDescriptor } from "../src/compiler.ts";
import { TypeScriptFactsService } from "../src/type-facts.ts";

void test("contract defaults deny builds while warn and off remain configurable", async () => {
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
      process.cwd(),
      {
        resolve: async () => null,
        read: async () => {
          throw new Error("unexpected read");
        },
        describe: (id) => ({
          ...fileModuleDescriptor(process.cwd(), id),
          uri: `untitled:${encodeURIComponent(id)}`,
        }),
        watch: () => {},
        warn: (message) => warnings.push(message),
      },
    );
    try {
      if (level === undefined) {
        await assert.rejects(promise, /macro-argument-mutation/);
      } else {
        assert.ok((await promise)?.code);
        assert.equal(warnings.length, level === "warn" ? 1 : 0);
      }
    } finally {
      facts.close();
    }
  }
});
