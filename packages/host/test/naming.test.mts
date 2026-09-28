import assert from "node:assert/strict";
import path from "node:path";
import test from "node:test";

import { NativeDesignSystemLoader, NativeProjectSession } from "@stillwind/native";

import { compileModule, fileModuleDescriptor } from "../src/compiler.ts";
import type { TypeScriptFactsService } from "../src/type-facts.ts";

void test("host name generators decide the names the compiler writes", async () => {
  const loader = new NativeDesignSystemLoader();
  loader.provide(0, "design.css", "@utility flex {display:flex} @stillwind-slot utilities;");
  const seen: Array<{ hash: number; module: string }> = [];
  const session = new NativeProjectSession(loader, false, undefined, {
    utilityClass: (context, hash) => {
      seen.push({ hash: hash.length, module: context.module });
      // A class a class attribute can hold, computed from the whole digest the host receives.
      return `app-${Buffer.from(hash).toString("hex").slice(0, 8)}`;
    },
  });
  const entry = path.resolve("named.tsx");
  const output = await compileModule(
    session,
    () => ({}) as TypeScriptFactsService,
    "import { style } from 'stillwind/react'; export const x = style('flex');",
    entry,
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
      warn: () => {},
    },
  );
  assert.ok(output?.code);
  // The host saw one request per generated class, with the whole digest and the module path.
  assert.equal(seen.length, 1);
  assert.equal(seen[0]!.hash, 32);
  assert.equal(seen[0]!.module, "named.tsx");
  const class_name = output.symbols.find((symbol) => symbol.kind === "class")!.name;
  assert.match(class_name, /^app-[0-9a-f]{8}$/);
  assert.ok(output.code.includes(class_name), output.code);
});

void test("a generator that fails only warns while the session is in development", async () => {
  const loader = new NativeDesignSystemLoader();
  loader.provide(0, "design.css", "@utility flex {display:flex} @stillwind-slot utilities;");
  const session = new NativeProjectSession(loader, false, "development", {
    utilityClass: () => {
      throw new Error("no name for this call");
    },
  });
  const warnings: string[] = [];
  const output = await compileModule(
    session,
    () => ({}) as TypeScriptFactsService,
    "import { style } from 'stillwind/react'; export const x = style('flex');",
    path.resolve("throwing.tsx"),
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
  // A development session keeps compiling with the built-in name and reports the rejection.
  assert.ok(output?.code);
  const class_name = output.symbols.find((symbol) => symbol.kind === "class")!.name;
  assert.match(class_name, /^sw_[0-9a-z]{5}$/);
  assert.equal(warnings.length, 1);
  assert.match(warnings[0]!, /class name/);
  assert.match(warnings[0]!, /no name for this call/);
});
