import assert from "node:assert/strict";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { runInNewContext } from "node:vm";

import { NativeDesignSystemLoader, NativeProjectSession } from "@stillwind/native";

import * as runtime from "../../core/src/runtime/react.ts";
import { compileModule } from "../src/compiler.ts";
import { TypeScriptFactsService } from "../src/type-facts.ts";

async function compile(source: string) {
  const root = path.dirname(fileURLToPath(import.meta.url));
  const loader = new NativeDesignSystemLoader();
  loader.provide(
    0,
    "design.css",
    "@utility red {color:red} @utility blue {color:blue} @utility green {color:green} @stillwind-slot utilities;",
  );
  const session = new NativeProjectSession(loader, false);
  const facts = new TypeScriptFactsService(root);
  try {
    const output = await compileModule(
      session,
      () => facts,
      `${source}\nglobalThis.result = { props, events };`,
      path.join(root, "entry.ts"),
      root,
      {
        resolve: async (specifier, importer) => ({
          id: path.resolve(path.dirname(importer), `${specifier}.ts`),
        }),
        read: async () => {
          throw new Error("unexpected dependency");
        },
        watch: () => {},
        warn: () => {},
      },
    );
    assert.ok(output);
    return output;
  } finally {
    facts.close();
  }
}

function evaluate(code: string, context: Record<string, unknown>) {
  const sandbox: Record<string, unknown> = { ...runtime, ...context };
  runInNewContext(code.replace(/^import\s+[^;]+;\s*/gm, ""), sandbox);
  return sandbox.result as {
    props: Record<string, unknown>;
    events: string[];
  };
}

void test("each conditional test is evaluated exactly once", async () => {
  const output = await compile(
    [
      "import {style} from 'stillwind/react';",
      "let calls = 0;",
      "const first = () => { calls += 1; return calls === 1; };",
      "const events = [];",
      "const props = style('red', first() ? 'blue' : 'green');",
      "events.push('calls:' + calls);",
    ].join("\n"),
  );
  const { props, events } = evaluate(output.code!, {});
  assert.deepEqual(Array.from(events), ["calls:1"]);
  assert.equal(String(props.className).split(" ").length, 2);
  assert.match(String(props.className), /_s_[0-9a-f]{8}$/);
});

void test("short-circuiting keeps a nested test from running", async () => {
  const output = await compile(
    [
      "import {style} from 'stillwind/react';",
      "let nested = 0;",
      "const second = () => { nested += 1; return true; };",
      "const events = [];",
      "const props = style('red', enabled && (second() ? 'blue' : 'green'));",
      "events.push('nested:' + nested);",
    ].join("\n"),
  );
  const disabled = evaluate(output.code!, { enabled: false });
  assert.deepEqual(Array.from(disabled.events), ["nested:0"]);
  assert.equal(String(disabled.props.className).split(" ").length, 1);
  const enabled = evaluate(output.code!, { enabled: true });
  assert.deepEqual(Array.from(enabled.events), ["nested:1"]);
  assert.equal(String(enabled.props.className).split(" ").length, 2);
});

void test("attribute values and class guards keep their source order", async () => {
  const output = await compile(
    [
      "import {style} from 'stillwind/react';",
      "const events = [];",
      "const props = style(",
      "  { 'aria-busy': (events.push('attribute'), true) },",
      "  (events.push('guard'), enabled) ? 'blue' : 'green',",
      ");",
    ].join("\n"),
  );
  const { props, events } = evaluate(output.code!, { enabled: true });
  assert.deepEqual(Array.from(events), ["attribute", "guard"]);
  assert.equal(props["aria-busy"], true);
  assert.equal(String(props.className).split(" ").length, 1);
});

void test("tier 1 and tier 2 produce the same classes", async () => {
  const tier1 = await compile(
    [
      "import {style} from 'stillwind/react';",
      "const events = [];",
      "const props = style('red', enabled && (nested ? 'blue' : 'green'));",
    ].join("\n"),
  );
  const tier2 = await compile(
    [
      "import {style} from 'stillwind/react';",
      "const base = style('red');",
      "const events = [];",
      "const props = style(base, enabled && (nested ? 'blue' : 'green'));",
    ].join("\n"),
  );
  assert.doesNotMatch(tier1.code!, /stillwindMerge/);
  assert.match(tier2.code!, /stillwindMerge\(stillwindToFragment\(\(base\)\)/);
  const bases = { tier1: new Set<string>(), tier2: new Set<string>() };
  for (const context of [
    { enabled: false, nested: false },
    { enabled: true, nested: false },
    { enabled: true, nested: true },
  ]) {
    const first = evaluate(tier1.code!, { ...context });
    const second = evaluate(tier2.code!, { ...context });
    // Each call owns its class identity, so compare the selected structure.
    const firstClasses = String(first.props.className).split(" ");
    const secondClasses = String(second.props.className).split(" ");
    assert.equal(secondClasses.length, firstClasses.length, JSON.stringify(context));
    assert.equal(secondClasses.length > 1, firstClasses.length > 1, JSON.stringify(context));
    // The unconditional base unit stays first and stable in both tiers.
    bases.tier1.add(firstClasses[0]);
    bases.tier2.add(secondClasses[0]);
  }
  assert.equal(bases.tier1.size, 1);
  assert.equal(bases.tier2.size, 1);
});

void test("fragments stay inside the call expression", async () => {
  const output = await compile(
    [
      "import {style} from 'stillwind/react';",
      "const events = [];",
      "const props = style({ 'aria-busy': true }, enabled ? 'blue' : 'green', nested && 'red');",
    ].join("\n"),
  );
  assert.doesNotMatch(output.code!, /Symbol\(/);
  const nonImport = output
    .code!.split("\n")
    .filter((line) => !line.startsWith("import "))
    .join("\n");
  const keyUses = nonImport.match(/stillwindClassKey/g)?.length ?? 0;
  assert.equal(nonImport.match(/\[stillwindClassKey\]/g)?.length ?? 0, keyUses);
  const { props } = evaluate(output.code!, { enabled: true, nested: false });
  assert.equal(props["aria-busy"], true);
  assert.equal(String(props.className).split(" ").length, 1);
});
