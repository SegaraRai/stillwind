import assert from "node:assert/strict";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { runInNewContext } from "node:vm";

import { NativeDesignSystemLoader, NativeProjectSession } from "@stillwind/native";

import * as runtime from "../../core/src/runtime/react.ts";
import { compileModule, fileModuleDescriptor } from "../src/compiler.ts";
import { TypeScriptFactsService } from "../src/type-facts.ts";

async function compile(files: Record<string, string>, entry = "entry.ts") {
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
      files[entry],
      path.join(root, entry),
      root,
      {
        resolve: async (specifier, importer) => ({
          id: path.resolve(path.dirname(importer), `${specifier}.ts`),
        }),
        read: async (id) => {
          const source = files[path.basename(id)];
          assert.notEqual(source, undefined);
          return source;
        },
        describe: (id) => ({
          ...fileModuleDescriptor(root, id),
          uri: `untitled:${encodeURIComponent(id)}`,
        }),
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

void test("nested transition branches resolve both imported producers", async () => {
  const output = await compile({
    "entry.ts": [
      "import {style} from 'stillwind/react';\nimport {a,b} from './barrel';",
      "const enabled = Boolean(globalThis.enabled), outer = Boolean(globalThis.outer);",
      "globalThis.props = style('green', outer ? (enabled && a.for('a')) : b.for('b'));",
    ].join("\n"),
    "barrel.ts": "export {a,b} from './transitions';",
    "transitions.ts":
      "import {style} from 'stillwind/react'; export const a=style.viewTransition({old:'red'}), b=style.viewTransition({old:'blue'});",
  });
  assert.doesNotMatch(output.code!, /\.for\(/);
  const classes = [];
  for (const context of [
    { outer: true, enabled: false },
    { outer: true, enabled: true },
    { outer: false, enabled: false },
  ]) {
    const sandbox = { ...runtime, ...context, props: { className: "" } };
    runInNewContext(output.code!.replace(/^import\s+[^;]+;\s*/gm, ""), sandbox);
    classes.push(sandbox.props.className.split(" "));
  }
  assert.deepEqual(
    classes.map((value) => value.length),
    [1, 2, 2],
  );
  assert.notEqual(classes[1].join(), classes[2].join());
});
void test("enumerates factory and mutable conditional bases and preserves evaluation order", async () => {
  const output = await compile({
    "entry.ts": `import {style} from 'stillwind/react';
const a=style('red');const b=style('green');const events=[];
const select=()=>{events.push('base');return flag?a:b;};
const props=style({class:(events.push('external'),'outside')},select(),'blue',{'data-x':(events.push('attribute'),1)});
globalThis.result={props,events};`,
  });
  const context: Record<string, unknown> = { ...runtime, flag: false };
  runInNewContext(output.code!.replace(/^import\s+[^;]+;\s*/gm, ""), context);
  const result = context.result as { events: string[]; props: Record<string, unknown> };
  assert.deepEqual(Array.from(result.events), ["external", "base", "attribute"]);
  assert.equal(result.props["data-x"], 1);
  assert.equal(String(result.props.className).split(" ").length, 3);
  assert.equal(JSON.parse(output.compositionTopology).length, 2);
  await assert.rejects(
    compile({
      "entry.ts": `import {style} from 'stillwind/react';const a=style('red');const b=style('green',{'data-x':1});let selected: typeof a | typeof b=a;if(flag)selected=b;const props=style(selected,'blue');`,
    }),
    /SW06005/,
  );
});

void test("rejects a CSS module cycle even when the style graph is acyclic", async () => {
  await assert.rejects(
    compile({
      "entry.ts":
        "import {style} from 'stillwind/react';import {middle} from './middle';export const base=style('red');export const derived=style(middle,'blue');",
      "middle.ts":
        "import {style} from 'stillwind/react';import {base} from './entry';export const middle=style(base,'green');",
    }),
    /SW06003/,
  );
});

void test("retains mutable candidates through exports and factories", async () => {
  await assert.rejects(
    compile({
      "entry.ts": `import {style} from 'stillwind/react';const a=style('red');const b=style('green',{'data-x':1});let selected: typeof a | typeof b=a;if(flag)selected=b;function select(){return selected;}const props=style(select(),'blue');`,
    }),
    /SW06005/,
  );
  await assert.rejects(
    compile({
      "entry.ts":
        "import {style} from 'stillwind/react';import {selected} from './base';const props=style(selected,'blue');",
      "base.ts":
        "import {style} from 'stillwind/react';const a=style('red');const b=style('green',{'data-x':1});export let selected: typeof a | typeof b=a;if(flag)selected=b;",
    }),
    /SW06005/,
  );
});

void test("parameterized factories preserve all base origins", async () => {
  await assert.rejects(
    compile({
      "entry.ts":
        "import {style} from 'stillwind/react';const a=style('red');const b=style('green',{'data-x':1});function select(flag){return flag?a:b;}const props=style(select(flag),'blue');",
    }),
    /SW06005/,
  );
  const output = await compile({
    "entry.ts":
      "import {style} from 'stillwind/react';const a=style('red'),b=style('green');function identity<T>(value:T){return value;}const props=style(identity(flag?a:b),'blue');",
  });
  assert.equal(output.composition.length, 2);
  await assert.rejects(
    compile({
      "entry.ts":
        "import {style} from 'stillwind/react';const a=style('red',{'data-x':1}),b=style('green');const choice:{value:typeof a|typeof b}={value:a};if(flag)choice.value=b;const props=style(choice.value,'blue');",
    }),
    /SW06005/,
  );
});

void test("unrelated JavaScript cycles do not become CSS ordering errors", async () => {
  const output = await compile({
    "entry.ts":
      "import {style} from 'stillwind/react';import {base} from './base';export function value(){return 1};export const props=style(base,'blue');",
    "base.ts":
      "import {style} from 'stillwind/react';import {value} from './entry';export function other(){return value()};export const base=style('red');",
  });
  assert.match(output.code!, /stillwindMerge/);
});

void test("conditional utilities keep their branches alongside a runtime-selected base", async () => {
  const output = await compile({
    "entry.ts":
      "import {style} from 'stillwind/react';const a=style('red'),b=style('green');const props=style(selectA?a:b,enabled?'blue':'red');globalThis.result={a,b,props};",
  });
  assert.equal(output.classes.length, 4);
  for (const selectA of [false, true]) {
    for (const enabled of [false, true]) {
      const context: Record<string, unknown> = { ...runtime, selectA, enabled };
      runInNewContext(output.code!.replace(/^import\s+[^;]+;\s*/gm, ""), context);
      const result = context.result as Record<string, { className: string }>;
      const classes = result.props.className.split(" ");
      assert.equal(classes.length, 2);
      assert.equal(classes[0], result[selectA ? "a" : "b"].className);
      assert.notEqual(classes[0], classes[1]);
    }
  }
});

void test("rejects unknown, props-bearing, nested, and incompatible-framework bases", async () => {
  for (const [body, error] of [
    ["const base={className:'user'};const props=style(base,'blue');", /SW03001/],
    [
      "const a=style('red');const b=style('green',{'data-x':1});const merged=style(a,b);const props=style(merged,'blue');",
      /SW06005/,
    ],
    ["const props=style({'data-x':style('red')});", /SW06001/],
  ] as const) {
    await assert.rejects(
      compile({ "entry.ts": `import {style} from 'stillwind/react';${body}` }),
      error,
    );
  }
  await assert.rejects(
    compile({
      "entry.ts":
        "import {style} from 'stillwind/react';import {base} from './base';const props=style(base,'blue');",
      "base.ts": "import {style} from 'stillwind/solid';export const base=style('red');",
    }),
    /SW06006/,
  );
});

void test("composition follows imported and re-exported style producers", async () => {
  const root = path.dirname(fileURLToPath(import.meta.url));
  const files = new Map([
    [
      "entry.ts",
      "import {style} from 'stillwind/react';import {a,b} from './barrel';export const c=style(flag?a:b,'blue');",
    ],
    ["barrel.ts", "export {a,b} from './base';"],
    [
      "base.ts",
      "import {style} from 'stillwind/react';export const a=style('red');export const b=style('green');",
    ],
  ]);
  const loader = new NativeDesignSystemLoader();
  loader.provide(
    0,
    "design.css",
    "@utility red {color:red} @utility blue {color:blue} @utility green {color:green} @stillwind-slot utilities;",
  );
  const session = new NativeProjectSession(loader, false);
  const facts = new TypeScriptFactsService(root);
  try {
    const compile = () =>
      compileModule(
        session,
        () => facts,
        files.get("entry.ts")!,
        path.join(root, "entry.ts"),
        root,
        {
          resolve: async (specifier, importer) => ({
            id: path.resolve(path.dirname(importer), `${specifier}.ts`),
          }),
          read: async (id) => files.get(path.basename(id))!,
          describe: (id) => ({
            ...fileModuleDescriptor(root, id),
            uri: `untitled:${encodeURIComponent(id)}`,
          }),
          watch: () => {},
          warn: () => {},
        },
      );
    const result = await compile();
    assert.ok(result);
    assert.match(
      result.code!,
      /\(flag\) \? \(stillwindToFragment\(\(a\)\)\) : \(stillwindToFragment\(\(b\)\)\)/,
    );
    assert.match(result.css, /color:blue/);
    assert.doesNotMatch(result.css, /color:red|color:green/);
    files.set(
      "base.ts",
      "import {style} from 'stillwind/react';export const a=style('red',{'data-x':1});export const b=style('green');",
    );
    await assert.rejects(compile(), /SW06005/);
  } finally {
    facts.close();
  }
});

void test("JavaScript checker shadows preserve JSDoc identities and reject incompatible assignment", async () => {
  const output = await compile(
    {
      "entry.js": `import {style} from 'stillwind/react';const a=style('red',/* retained */),b=style('green');
/**
 * @template T
 * @param {T} value
 * @returns {T}
 */ function identity(value){return value;} const props=style(identity(Math.random()?a:b),'blue');`,
    },
    "entry.js",
  );
  assert.equal(output.composition.length, 2);
  assert.doesNotMatch(output.code!, /__stillwindDeclarationIdentity/);
  await assert.rejects(
    compile(
      {
        "entry.js": `import {style} from 'stillwind/react';const a=style('red'),b=style('green');let selected=a;if(Math.random())selected=b;const props=style(selected,'blue');`,
      },
      "entry.js",
    ),
    /SHTS002/,
  );
});
void test("checker identity insertion accepts empty commented calls and trailing commas", async () => {
  const output = await compile({
    "entry.ts": `import {style} from 'stillwind/react';const r=style.ref(/* marker */);const a=style('red',/* trailing */);const props=style(a,'blue',);const target=style(r);`,
  });
  assert.equal(output.composition.length, 1);
});
