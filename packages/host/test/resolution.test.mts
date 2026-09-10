import assert from "node:assert/strict";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { NativeDesignSystemLoader, NativeProjectSession } from "@stillwind/native";

import { compileModule } from "../src/compiler.ts";
import { TypeScriptFactsService } from "../src/type-facts.ts";

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
      "import {style} from 'stillwind/react'; export const token=style.animation({from:'red',to:'red'});",
    ],
    [
      changedToken,
      "import {style} from 'stillwind/react'; export const token=style.animation({from:'blue',to:'blue'});",
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
        watch: () => {},
        warn: () => {},
      });
    const output = await compile();
    assert.ok(output);
    assert.match(output.css, /@keyframes/);
    assert.match(output.css, /color:red/);
    assert.deepEqual(reads, [barrel, token]);
    assert.ok(output.symbols.some((symbol) => symbol.source_path === "aliased/actual-token.ts"));
    selected = changedToken;
    const updated = await compile();
    assert.ok(updated);
    assert.match(updated.css, /color:blue/);
    assert.doesNotMatch(updated.css, /color:red/);
    assert.ok(updated.symbols.some((symbol) => symbol.source_path === "changed.ts"));
    assert.equal(facts.stats.incrementalSnapshots, 1);
  } finally {
    facts.close();
  }
});
