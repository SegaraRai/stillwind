import { mkdtempSync, writeFileSync, rmSync, mkdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { summarizeSource, NativeDesignSystemLoader, NativeProjectSession } from "@stillwind/native";
import { expect, test } from "vite-plus/test";

import { cleanModuleId, compileModule, fileModuleDescriptor } from "../src/compiler.ts";
import { TypeScriptFactsService } from "../src/type-facts.ts";

test("distinct resolver identities cannot share a module identity", async () => {
  const root = path.dirname(fileURLToPath(import.meta.url));
  const entry = path.join(root, "collision-entry.ts");
  const dependency = path.join(root, "collision-dependency.ts");
  const loader = new NativeDesignSystemLoader();
  loader.provide(0, "design.css", "@utility red {color:red} @stillwind-slot utilities;");
  const session = new NativeProjectSession(loader, false);
  await expect(
    compileModule(
      session,
      () => {
        expect.fail("type facts must not be requested after an identity collision");
      },
      "import {style} from 'stillwind/react'; import {token} from './token'; export const x=style(token);",
      entry,
      root,
      {
        describe: (id) => ({ ...fileModuleDescriptor(root, id), moduleId: "shared.ts" }),
        resolve: async () => ({ id: dependency }),
        read: async () => "export const token = 'red';",
        watch: () => {},
        warn: () => {},
      },
    ),
  ).rejects.toThrow(/Module identity shared\.ts is shared/);
});

test("virtual resolver sub-identities survive request cleaning", () => {
  expect(cleanModuleId("\0virtual:component?region=2")).toBe("\0virtual:component?region=2");
  expect(cleanModuleId("virtual:component?region=2")).toBe("virtual:component?region=2");
  expect(cleanModuleId("component.ts?t=123")).toBe("component.ts");
});

test("checker follows per-importer host aliases and barrel resolutions", async () => {
  const root = path.dirname(fileURLToPath(import.meta.url));
  const entry = path.join(root, "alias-entry.ts");
  const barrel = path.join(root, "aliased/barrel.ts");
  const token = path.join(root, "aliased/actual-token.ts");
  const changedToken = path.join(root, "changed.ts");
  let selected = token;
  const source =
    "import {style} from 'stillwind/react';\n// 日本語 🧭\nconst local=style.ref();\nimport {token} from '@chosen';\nexport const props=style(token(''));";
  const files = new Map([
    [barrel, "export {token} from '@chosen';"],
    [
      token,
      "import {style} from 'stillwind/react'; export const token=style.keyframes({from:'red',to:'red'});",
    ],
    [
      changedToken,
      "import {style} from 'stillwind/react'; export const token=style.keyframes({from:'blue',to:'blue'});",
    ],
  ]);
  const loader = new NativeDesignSystemLoader();
  loader.provide(
    0,
    "design.css",
    "@utility red {color:red} @utility blue {color:blue} @stillwind-slot utilities;",
  );
  const session = new NativeProjectSession(loader, false);
  const facts = new TypeScriptFactsService(root);
  const reads: string[] = [];
  try {
    const compile = () =>
      compileModule(session, () => facts, source, entry, root, {
        resolve: async (specifier, importer) => {
          expect(specifier).toBe("@chosen");
          return { id: importer === entry ? barrel : selected };
        },
        read: async (id) => {
          reads.push(id);
          expect(files.has(id)).toBeTruthy();
          return files.get(id)!;
        },
        describe: (id) => ({
          ...fileModuleDescriptor(root, id),
          document: {
            ...fileModuleDescriptor(root, id).document,
            uri: `untitled:${encodeURIComponent(id)}`,
          },
        }),
        watch: () => {},
        warn: () => {},
      });
    const output = await compile();
    expect(output).toBeTruthy();
    expect(output!.css).toMatch(/@keyframes/);
    expect(output!.css).toMatch(/color:red/);
    expect(reads).toStrictEqual([barrel, token]);
    expect(
      output!.symbols.some(
        (symbol) => symbol.sourceKind === "module" && symbol.sourceId === "aliased/actual-token.ts",
      ),
    ).toBeTruthy();
    selected = changedToken;
    const updated = await compile();
    expect(updated).toBeTruthy();
    expect(updated!.css).toMatch(/color:blue/);
    expect(updated!.css).not.toMatch(/color:red/);
    expect(
      updated!.symbols.some(
        (symbol) => symbol.sourceKind === "module" && symbol.sourceId === "changed.ts",
      ),
    ).toBeTruthy();
    expect(facts.stats.incrementalSnapshots).toBe(1);
  } finally {
    facts.close();
  }
});

test("disk dependencies gain and lose semantic overlays across snapshots", () => {
  const root = mkdtempSync(
    path.join(path.dirname(fileURLToPath(import.meta.url)), ".test-disk-overlay-"),
  );
  const entrySource =
    "import {style} from 'stillwind/react';import {token} from './token';export const props=style(token);";
  const tokenSource =
    "import {style} from 'stillwind/react';export const token=style.ref('token');";
  const source = (name: string, text: string) => {
    const summary = summarizeSource(
      text,
      name,
      name.endsWith(".tsx") ? "tsx" : "ts",
      "unambiguous",
    );
    writeFileSync(path.join(root, name), text);
    return {
      resolverId: path.join(root, name),
      checkerFileName: path.join(root, name),
      moduleId: name,
      projectionFileName: path.join(root, name),
      kind: "direct" as const,
      document: {
        uri: `untitled:${name}`,
        code: text,
        format: name.endsWith(".tsx") ? ("jsx" as const) : ("script" as const),
      },
      syntax: {
        language: name.endsWith(".tsx") ? ("tsx" as const) : ("ts" as const),
        moduleKind: "unambiguous" as const,
      },
      probes: summary.typeProbes,
      assets: summary.assets,
    };
  };
  const entry = source("entry.ts", entrySource);
  const token = source("token.ts", tokenSource);
  const service = new TypeScriptFactsService(root);
  try {
    const collect = (includeToken: boolean) => {
      const sources = includeToken ? [entry, token] : [entry];
      return service
        .collect(sources, new Map(sources.map((source) => [source.moduleId, source.assets])))
        .get("entry.ts")!;
    };
    expect(() => collect(false)).toThrow(/SHTS003/);
    expect(collect(true).length).toBe(1);
    expect(() => collect(false)).toThrow(/SHTS003/);
    expect(collect(true).length).toBe(1);
    expect(service.stats.restarts).toBe(0);
    expect(service.stats.incrementalSnapshots).toBe(3);
  } finally {
    service.close();
    rmSync(root, { recursive: true, force: true });
  }
});

test("external package runtime exports do not replace declaration resolution", async () => {
  const root = mkdtempSync(
    path.join(path.dirname(fileURLToPath(import.meta.url)), ".test-package-types-"),
  );
  const packageRoot = path.join(root, "node_modules/external-types");
  mkdirSync(packageRoot, { recursive: true });
  writeFileSync(
    path.join(packageRoot, "package.json"),
    JSON.stringify({
      name: "external-types",
      exports: { types: "./index.d.ts", default: "./index.js" },
    }),
  );
  writeFileSync(path.join(packageRoot, "index.d.ts"), "export type Props<T> = T;");
  writeFileSync(path.join(packageRoot, "index.js"), "export const runtime = true;");
  const source =
    "import type {Props} from 'external-types';import {style} from 'stillwind/react';const a=style.ref('a'),b=style.ref('b');export function component({alternate=false}:Props<{alternate?:boolean}>){const selected=alternate?b:a;return style(selected)}";
  const loader = new NativeDesignSystemLoader();
  loader.provide(0, "root.css", "@stillwind-slot utilities;");
  const session = new NativeProjectSession(loader, false);
  const service = new TypeScriptFactsService(root);
  try {
    const output = await compileModule(
      session,
      () => service,
      source,
      path.join(root, "entry.ts"),
      root,
      {
        resolve: async () => ({ id: path.join(packageRoot, "index.js") }),
        read: async () =>
          expect.fail("external runtime implementation must not enter semantic inputs"),
        describe: (id) => ({
          ...fileModuleDescriptor(root, id),
          document: {
            ...fileModuleDescriptor(root, id).document,
            uri: `untitled:${encodeURIComponent(id)}`,
          },
        }),
        watch() {},
        warn() {},
      },
    );
    expect(output).toBeTruthy();
    expect(output!.diagnostics).toStrictEqual([]);
    expect(output!.code!).toMatch(/\(selected\)\.c/);
  } finally {
    service.close();
    rmSync(root, { recursive: true, force: true });
  }
});
