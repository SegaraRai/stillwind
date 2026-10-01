import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync, rmSync, mkdirSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { summarizeSource, NativeDesignSystemLoader, NativeProjectSession } from "@stillwind/native";

import { cleanModuleId, compileModule, fileModuleDescriptor } from "../src/compiler.ts";
import { TypeScriptFactsService } from "../src/type-facts.ts";

void test("distinct resolver identities cannot share a module identity", async () => {
  const root = path.dirname(fileURLToPath(import.meta.url));
  const entry = path.join(root, "collision-entry.ts");
  const dependency = path.join(root, "collision-dependency.ts");
  const loader = new NativeDesignSystemLoader();
  loader.provide(0, "design.css", "@utility red {color:red} @stillwind-slot utilities;");
  const session = new NativeProjectSession(loader, false);
  await assert.rejects(
    compileModule(
      session,
      () => {
        throw new Error("type facts must not be requested after an identity collision");
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
    /Module identity shared\.ts is shared/,
  );
});

void test("virtual resolver sub-identities survive request cleaning", () => {
  assert.equal(cleanModuleId("\0virtual:component?region=2"), "\0virtual:component?region=2");
  assert.equal(cleanModuleId("virtual:component?region=2"), "virtual:component?region=2");
  assert.equal(cleanModuleId("component.ts?t=123"), "component.ts");
});

void test("checker follows per-importer host aliases and barrel resolutions", async () => {
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
          assert.equal(specifier, "@chosen");
          return { id: importer === entry ? barrel : selected };
        },
        read: async (id) => {
          reads.push(id);
          assert.ok(files.has(id));
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
    assert.ok(output);
    assert.match(output.css, /@keyframes/);
    assert.match(output.css, /color:red/);
    assert.deepEqual(reads, [barrel, token]);
    assert.ok(
      output.symbols.some(
        (symbol) => symbol.sourceKind === "module" && symbol.sourceId === "aliased/actual-token.ts",
      ),
    );
    selected = changedToken;
    const updated = await compile();
    assert.ok(updated);
    assert.match(updated.css, /color:blue/);
    assert.doesNotMatch(updated.css, /color:red/);
    assert.ok(
      updated.symbols.some(
        (symbol) => symbol.sourceKind === "module" && symbol.sourceId === "changed.ts",
      ),
    );
    assert.equal(facts.stats.incrementalSnapshots, 1);
  } finally {
    facts.close();
  }
});

void test("disk dependencies gain and lose semantic overlays across snapshots", () => {
  const root = mkdtempSync(
    path.join(path.dirname(fileURLToPath(import.meta.url)), ".disk-overlay-"),
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
    assert.throws(() => collect(false), /SHTS003/);
    assert.equal(collect(true).length, 1);
    assert.throws(() => collect(false), /SHTS003/);
    assert.equal(collect(true).length, 1);
    assert.equal(service.stats.restarts, 0);
    assert.equal(service.stats.incrementalSnapshots, 3);
  } finally {
    service.close();
    rmSync(root, { recursive: true, force: true });
  }
});

void test("external package runtime exports do not replace declaration resolution", async () => {
  const root = mkdtempSync(
    path.join(path.dirname(fileURLToPath(import.meta.url)), ".package-types-"),
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
          assert.fail("external runtime implementation must not enter semantic inputs"),
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
    assert.ok(output);
    assert.deepEqual(output.diagnostics, []);
    assert.match(output.code!, /\(selected\)\.c/);
  } finally {
    service.close();
    rmSync(root, { recursive: true, force: true });
  }
});
