import assert from "node:assert/strict";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { NativeDesignSystemLoader, NativeProjectSession } from "@stillwind/native";

import { identityKey } from "../src/checker-source.ts";
import { compileModule, fileModuleDescriptor } from "../src/compiler.ts";
import { TypeScriptFactsService, TypeFactsError } from "../src/type-facts.ts";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../integration/test");

void test("checker declaration identity has its own namespace", () => {
  const key = identityKey({
    kind: "ref",
    moduleId: "src/a.tsx",
    start: 2,
    end: 6,
    bindingName: "item",
  });
  assert.equal(
    Buffer.from(key.slice("stillwind-identity/v1:".length), "base64url").toString(),
    '{"kind":"ref","moduleId":"src/a.tsx","start":2,"end":6,"bindingName":"item"}',
  );
  assert.equal(
    identityKey({ bindingName: "item", end: 6, start: 2, moduleId: "src/a.tsx", kind: "ref" }),
    key,
  );
  assert.match(key, /^stillwind-identity\/v1:/);
});

function wrap(ext: string, script: string, template = "", module = false) {
  if (ext === "astro") {
    return `---\n${script}\n---\n${template}`;
  }
  if (ext === "vue") {
    return `<script ${module ? "" : "setup"} lang="ts">\n${script}\n</script>\n<template>${template}</template>`;
  }
  return `<script ${module ? "module" : ""} lang="ts">\n${script}\n</script>\n${template}`;
}
function harness(t: test.TestContext, files: Record<string, string>, entry: string) {
  const loader = new NativeDesignSystemLoader();
  loader.provide(
    0,
    "theme.css",
    "@utility red {color:red;} @utility blue {color:blue;} @utility green {color:green;} @stillwind-slot utilities;",
  );
  const session = new NativeProjectSession(loader, true),
    facts = new TypeScriptFactsService(root);
  t.after(() => facts.close());
  const compile = (options: { diagnostics?: "collect" } = {}) =>
    compileModule(
      session,
      () => facts,
      files[entry],
      path.join(root, entry),
      root,
      {
        resolve: async (specifier, importer) =>
          specifier.startsWith(".")
            ? { id: path.resolve(path.dirname(importer), specifier) }
            : null,
        read: async (id) => {
          const text = files[path.relative(root, id)];
          assert.notEqual(text, undefined, id);
          return text;
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
      },
      options,
    );
  return { compile, session, facts };
}
for (const ext of ["vue", "svelte", "astro"]) {
  void test(`${ext}: transitive SFC declarations, generics and original Unicode identity survive edits`, async (t) => {
    const entry = `Typed.${ext}`,
      producer = `Base.${ext}`;
    const script = `// 日本語 🧭\nimport {style} from 'stillwind/${ext}';\nimport {base} from './barrel.ts';\nimport {identity} from './helper.ts';\nconst derived=style(identity(base),'blue');`;
    const template = ext === "vue" ? '<div v-bind="derived" />' : "<div {...derived}></div>";
    const files = {
      [entry]: wrap(ext, script, template),
      [producer]: wrap(
        ext,
        `// 日本語 🧭\nimport {style} from 'stillwind/${ext}'; export const base=style('red');`,
        "",
        true,
      ),
      "barrel.ts": `export {base} from './${producer}';`,
      "helper.ts": "export function identity<T>(value:T):T{return value;}",
    };
    const { compile, session, facts } = harness(t, files, entry);
    const first = await compile();
    assert.ok(first?.code);
    assert.deepEqual(first.diagnostics, []);
    assert.equal(first.composition.length, 1);
    const base = first.composition[0].base;
    assert.equal(base.moduleId, producer);
    assert.equal(
      Buffer.from(files[producer]).subarray(base.start, base.end).toString(),
      "style('red')",
    );
    assert.match(session.moduleCss(entry), /color:\s*blue/);
    assert.doesNotMatch(first.code, /stillwind-identity\/v1:|__stillwindDeclarationIdentity/);
    files[producer] = files[producer].replace("'red'", "'green'");
    const second = await compile();
    assert.ok(second?.code);
    assert.equal(
      Buffer.from(files[producer])
        .subarray(second.composition[0].base.start, second.composition[0].base.end)
        .toString(),
      "style('green')",
    );
    assert.equal(facts.stats.restarts, 0);
    assert.equal(facts.stats.incrementalSnapshots, 1);
  });
  void test(`${ext}: incompatible reassignment is an original-coordinate compile error`, async (t) => {
    const entry = `Invalid.${ext}`;
    const script = `// 日本語 🧭\nimport {style} from 'stillwind/${ext}';const a=style('red'),b=style('green');let selected=a;if(Math.random())selected=b;const derived=style(selected,'blue');`;
    const files = { [entry]: wrap(ext, script) };
    await assert.rejects(harness(t, files, entry).compile(), (error: unknown) => {
      assert.ok(error instanceof TypeFactsError);
      assert.equal(error.code, "SHTS002");
      assert.equal(error.moduleId, entry);
      assert.equal(
        Buffer.from(files[entry]).subarray(error.start, error.end).toString(),
        "selected",
      );
      return true;
    });
  });
  void test(`${ext}: explicit unions retain candidates and erased identity is rejected`, async (t) => {
    const entry = `Union.${ext}`;
    const script = `import {style} from 'stillwind/${ext}';const a=style('red'),b=style('green');let selected:typeof a|typeof b=a;if(Math.random())selected=b;const derived=style(selected,'blue');`;
    const files = { [entry]: wrap(ext, script) };
    const { compile } = harness(t, files, entry);
    const result = await compile();
    assert.equal(result?.composition.length, 2);
    files[entry] = wrap(ext, script.replace("typeof a|typeof b", "any"));
    await assert.rejects(compile(), /SHTS003/);
  });
}
void test("Astro client imports receive their own typed scope", async (t) => {
  const files = {
    "Client.astro": `<script>\n// 🧭\nimport {style} from 'stillwind/astro';import {base} from './client-base.ts';const props=style(base,'blue');console.log(props);\n</script>`,
    "client-base.ts": `import {style} from 'stillwind/astro';export const base=style('red');`,
  };
  const result = await harness(t, files, "Client.astro").compile();
  assert.ok(result?.code);
  assert.equal(result.composition.length, 1);
  assert.deepEqual(result.diagnostics, []);
});

for (const ext of ["vue", "svelte", "astro"]) {
  void test(`${ext}: framework props and template loops retain typed candidates`, async (t) => {
    const entry = `Template.${ext}`;
    let script = `import {style} from 'stillwind/${ext}';const a=style('red'),b=style('green');const choices=[a,b];`;
    let template = "";
    if (ext === "vue") {
      script += `const {enabled}=defineProps<{enabled:boolean}>();`;
      template = `<div v-bind="style(enabled?a:b,'blue')"/><div v-for="item in choices" v-bind="style(item,'blue')"/>`;
    }
    if (ext === "svelte") {
      script += `let {enabled}:{enabled:boolean}=$props();let reactive=$state(a);import {writable} from 'svelte/store';const store=writable(enabled?a:b);`;
      template = `<div {...style($store,'blue')}></div>{#each choices as item}<div {...style(item,'blue')}></div>{/each}<div {...style(reactive,'blue')}></div>`;
    }
    if (ext === "astro") {
      script += `interface Props {enabled:boolean} const {enabled}=Astro.props;`;
      template = `<div {...style(enabled?a:b,'blue')}></div>{choices.map(item=><div {...style(item,'blue')}/> )}`;
    }
    const files = { [entry]: wrap(ext, script, template) };
    const result = await harness(t, files, entry).compile();
    assert.ok(result?.code);
    assert.deepEqual(result.diagnostics, []);
    assert.equal(result.composition.length, ext === "svelte" ? 5 : 4);
  });
  void test(`${ext}: imported refs, keyframes and transitions keep original declaration positions`, async (t) => {
    const entry = `Assets.${ext}`,
      producer = `AssetsBase.${ext}`;
    const script = `import {style} from 'stillwind/${ext}';import {r,k,v} from './${producer}';const selected=k;const transition=v;const props=style(r,selected(''),transition);`;
    const files = {
      [entry]: wrap(ext, script),
      [producer]: wrap(
        ext,
        `// 日本語 🧭\nimport {style} from 'stillwind/${ext}';export const r=style.ref();export const k=style.keyframes({from:'red',to:'blue'});export const v=style.viewTransition({old:'red'});`,
        "",
        true,
      ),
    };
    const result = await harness(t, files, entry).compile();
    assert.ok(result?.code);
    assert.deepEqual(result.diagnostics, []);
    const assets = result.symbols.filter(
      (symbol) => symbol.sourceKind === "module" && symbol.sourceId === producer,
    );
    for (const symbol of assets) {
      assert.equal(symbol.sourceId, producer);
      assert.ok(
        ["r", "k", "v"].includes(
          Buffer.from(files[producer]).subarray(symbol.start, symbol.end).toString(),
        ),
        JSON.stringify(symbol),
      );
    }
    assert.ok(assets.length >= 3, JSON.stringify(result.symbols));
  });
}
void test("Astro identical client script bodies retain separate scopes", async (t) => {
  const body = `import {style} from 'stillwind/astro';import {base} from './same.ts';const props=style(base,'blue');`;
  const files = {
    "Duplicate.astro": `<script>${body}</script><script is:inline>${body}</script>`,
    "same.ts": `import {style} from 'stillwind/astro';export const base=style('red');`,
  };
  const result = await harness(t, files, "Duplicate.astro").compile();
  assert.ok(result?.code);
  assert.equal(result.composition.length, 2);
});

void test("SFC checker errors participate in collect mode with original editor coordinates", async (t) => {
  const entry = "Diagnostic.vue";
  const source = wrap(
    "vue",
    `// 日本語 🧭
import {style} from 'stillwind/vue';const a=style('red'),b=style('green');let selected=a;if(Math.random())selected=b;const props=style(selected);`,
  );
  const { compile } = harness(t, { [entry]: source }, entry);
  const result = await compile({ diagnostics: "collect" });
  assert.ok(result);
  assert.equal(result.code, null);
  const error = result.diagnostics[0];
  assert.equal(error.code, "SHTS002");
  assert.equal(source.slice(error.startUtf16, error.endUtf16), "selected");
  assert.equal(Buffer.from(source).subarray(error.start, error.end).toString(), "selected");
  assert.equal(error.startLine, 2);
});
