import path from "node:path";

import { NativeDesignSystemLoader, NativeProjectSession } from "@stillwind/native";
import { expect, test } from "vite-plus/test";

import { compileModule, fileModuleDescriptor } from "../src/compiler.ts";
import type { TypeScriptFactsService } from "../src/type-facts.ts";

test("host name generators decide the names the compiler writes", async () => {
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
      warn: () => {},
    },
  );
  expect(output?.code).toBeTruthy();
  // The host saw one request per generated class, with the whole digest and the module path.
  expect(seen.length).toBe(1);
  expect(seen[0]!.hash).toBe(32);
  expect(seen[0]!.module).toBe("named.tsx");
  const className = output!.symbols.find((symbol) => symbol.kind === "class")!.name;
  expect(className).toMatch(/^app-[0-9a-f]{8}$/);
  expect(output!.code!, output!.code!).toContain(className);
});

test("a generator that fails only warns while the session is in development", async () => {
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
  // A development session keeps compiling with the built-in name and reports the rejection.
  expect(output?.code).toBeTruthy();
  const className = output!.symbols.find((symbol) => symbol.kind === "class")!.name;
  expect(className).toMatch(/^sw_[0-9a-z]{5}$/);
  expect(warnings.length).toBe(1);
  expect(warnings[0]!).toMatch(/class name/);
  expect(warnings[0]!).toMatch(/no name for this call/);
});
