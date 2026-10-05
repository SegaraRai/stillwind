import path from "node:path";
import { fileURLToPath } from "node:url";

import { NativeDesignSystemLoader, NativeProjectSession } from "@stillwind/native";
import { expect, test, type TestContext } from "vite-plus/test";

import { compileModule, fileModuleDescriptor, CompilationFailure } from "../src/compiler.ts";
import { TypeScriptFactsService } from "../src/type-facts.ts";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../integration/testss");

function wrap(ext: string, script: string, template = "", module = false) {
  if (ext === "astro") {
    return `---\n${script}\n---\n${template}`;
  }
  if (ext === "vue") {
    return `<script ${module ? "" : "setup"} lang="ts">\n${script}\n</script>\n<template>${template}</template>`;
  }
  return `<script ${module ? "module" : ""} lang="ts">\n${script}\n</script>\n${template}`;
}
function harness(t: TestContext, files: Record<string, string>, entry: string) {
  const loader = new NativeDesignSystemLoader();
  loader.provide(
    0,
    "theme.css",
    "@utility red {color:red;} @utility blue {color:blue;} @utility green {color:green;} @stillwind-slot utilities;",
  );
  const session = new NativeProjectSession(loader, true),
    facts = new TypeScriptFactsService(root);
  t.onTestFinished(() => facts.close());
  const compile = (options: { diagnostics?: "collect" } = {}) =>
    compileModule(
      session,
      () => facts,
      files[entry],
      path.join(root, entry),
      {
        resolve: async (specifier, importer) =>
          specifier.startsWith(".")
            ? { id: path.resolve(path.dirname(importer), specifier) }
            : null,
        read: async (id) => {
          const text = files[path.relative(root, id)];
          expect(text, id).not.toBe(undefined);
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
  test(`${ext}: transitive SFC declarations, generics and original Unicode identity survive edits`, async (t) => {
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
    expect(first?.code).toBeTruthy();
    expect(first!.diagnostics).toStrictEqual([]);
    expect(first!.composition.length).toBe(1);
    const base = first!.composition[0].base;
    expect(base.moduleId).toBe(producer);
    expect(Buffer.from(files[producer]).subarray(base.start, base.end).toString()).toBe(
      "style('red')",
    );
    expect(session.moduleCss(entry)).toMatch(/color:\s*blue/);
    expect(first!.code).not.toMatch(/stillwind-identity\/v1:|__stillwindDeclarationIdentity/);
    files[producer] = files[producer].replace("'red'", "'green'");
    const second = await compile();
    expect(second?.code).toBeTruthy();
    expect(
      Buffer.from(files[producer])
        .subarray(second!.composition[0].base.start, second!.composition[0].base.end)
        .toString(),
    ).toBe("style('green')");
    expect(facts.stats.restarts).toBe(0);
    expect(facts.stats.incrementalSnapshots).toBe(1);
  });
  test(`${ext}: incompatible reassignment is an original-coordinate compile error`, async (t) => {
    const entry = `Invalid.${ext}`;
    const script = `// 日本語 🧭\nimport {style} from 'stillwind/${ext}';const a=style('red'),b=style('green');let selected=a;if(Math.random())selected=b;const derived=style(selected,'blue');`;
    const files = { [entry]: wrap(ext, script) };
    const compilation = harness(t, files, entry).compile();
    await expect(compilation).rejects.toBeInstanceOf(CompilationFailure);
    const error = await compilation.catch((error: unknown) => error);
    if (!(error instanceof CompilationFailure)) {
      expect.fail("expected a compilation failure");
    }
    const report = error.batches[0].reports[0];
    expect(report.code).toBe("SW12004");
    expect(error.batches[0].module).toBe(entry);
    expect(
      Buffer.from(files[entry])
        .subarray(report.primary.location.start, report.primary.location.end)
        .toString(),
    ).toBe("selected");
  });
  test(`${ext}: explicit unions retain candidates and erased identity is rejected`, async (t) => {
    const entry = `Union.${ext}`;
    const script = `import {style} from 'stillwind/${ext}';const a=style('red'),b=style('green');let selected:typeof a|typeof b=a;if(Math.random())selected=b;const derived=style(selected,'blue');`;
    const files = { [entry]: wrap(ext, script) };
    const { compile } = harness(t, files, entry);
    const result = await compile();
    expect(result?.composition.length).toBe(2);
    files[entry] = wrap(ext, script.replace("typeof a|typeof b", "any"));
    await expect(compile()).rejects.toThrow(/SW12005/);
  });
}
test("Astro client imports receive their own typed scope", async (t) => {
  const files = {
    "Client.astro": `<script>\n// 🧭\nimport {style} from 'stillwind/astro';import {base} from './client-base.ts';const props=style(base,'blue');console.log(props);\n</script>`,
    "client-base.ts": `import {style} from 'stillwind/astro';export const base=style('red');`,
  };
  const result = await harness(t, files, "Client.astro").compile();
  expect(result?.code).toBeTruthy();
  expect(result!.composition.length).toBe(1);
  expect(result!.diagnostics).toStrictEqual([]);
});

for (const ext of ["vue", "svelte", "astro"]) {
  test(`${ext}: framework props and template loops retain typed candidates`, async (t) => {
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
    expect(result?.code).toBeTruthy();
    expect(result!.diagnostics).toStrictEqual([]);
    expect(result!.composition.length).toBe(ext === "svelte" ? 5 : 4);
  });
  test(`${ext}: imported refs, keyframes and transitions keep original declaration positions`, async (t) => {
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
    expect(result?.code).toBeTruthy();
    expect(result!.diagnostics).toStrictEqual([]);
    const assets = result!.symbols.filter(
      (symbol) => symbol.sourceKind === "module" && symbol.sourceId === producer,
    );
    for (const symbol of assets) {
      expect(symbol.sourceId).toBe(producer);
      expect(["r", "k", "v"], JSON.stringify(symbol)).toContain(
        Buffer.from(files[producer]).subarray(symbol.start, symbol.end).toString(),
      );
    }
    expect(assets.length, JSON.stringify(result!.symbols)).toBeGreaterThanOrEqual(3);
  });
}
test("Astro identical client script bodies retain separate scopes", async (t) => {
  const body = `import {style} from 'stillwind/astro';import {base} from './same.ts';const props=style(base,'blue');`;
  const files = {
    "Duplicate.astro": `<script>${body}</script><script is:inline>${body}</script>`,
    "same.ts": `import {style} from 'stillwind/astro';export const base=style('red');`,
  };
  const result = await harness(t, files, "Duplicate.astro").compile();
  expect(result?.code).toBeTruthy();
  expect(result!.composition.length).toBe(2);
});

for (const reference of ["ref", "shallowRef"] as const) {
  for (const narrowed of [false, true]) {
    test(`Vue ${reference} preserves composition identity${narrowed ? " after narrowing" : ""}`, async (t) => {
      const entry = "Unwrapped.vue";
      const files = {
        [entry]: wrap(
          "vue",
          `// 日本語 🧭
import {style} from 'stillwind/vue';import {${reference}} from 'vue';
const base=style('red');const chosen=${reference}(base);`,
          `<div ${narrowed ? 'v-if="chosen" ' : ""}v-bind="style(chosen,'blue')"/>`,
        ),
      };
      const result = await harness(t, files, entry).compile();
      expect(result!.diagnostics).toStrictEqual([]);
      expect(result!.composition).toHaveLength(1);
      const identity = result!.composition[0].base;
      expect(Buffer.from(files[entry]).subarray(identity.start, identity.end).toString()).toBe(
        "style('red')",
      );
    });
  }
}

test("Vue generated helpers avoid authored helpers, globalThis, and escaped import names", async (t) => {
  const entry = "HelperNames.vue";
  const script = `import {style} from 'stillwind/vue';import {ref} from 'vue';
const base=style('red');const chosen=ref(base);
function __VLS_unwrap(value:unknown,witness?:unknown){return String(value);}
function __VLS_withDotValue(value:unknown,witness?:unknown){}
const globalThis={__VLS_unwrap,__VLS_withDotValue};
const __stillwindCheckerHelper\\u0030=1,__stillwindCheckerHelper1=2;`;
  const authored = "__VLS_unwrap('authored', {} as import('vue').Ref<unknown>)";
  const files = {
    [entry]: wrap(
      "vue",
      script,
      `<div v-if="chosen" v-bind="style(chosen,'blue',{class:${authored}})"/>`,
    ),
  };
  const result = await harness(t, files, entry).compile();
  expect(result!.diagnostics).toStrictEqual([]);
  expect(result!.composition).toHaveLength(1);
  expect(result!.code).toContain(authored);
  expect(result!.code).toContain("const globalThis={__VLS_unwrap,__VLS_withDotValue}");
});

test("Vue deep refs retain declaration roles for refs, keyframes, and view transitions", async (t) => {
  const entry = "UnwrappedAssets.vue";
  const files = {
    [entry]: wrap(
      "vue",
      `import {style} from 'stillwind/vue';import {ref} from 'vue';
const r=style.ref('target'),k=style.keyframes({from:'red',to:'blue'}),v=style.viewTransition({old:'red'});
const target=ref(r),animation=ref(k),transition=ref(v);`,
      `<div v-if="transition" v-bind="style(target,animation(''),transition)"/>`,
    ),
  };
  const result = await harness(t, files, entry).compile();
  expect(result!.diagnostics).toStrictEqual([]);
  expect(result!.code).toBeTruthy();
  expect(
    result!.symbols
      .filter((symbol) => symbol.sourceKind === "module" && symbol.sourceId === entry)
      .map((symbol) => Buffer.from(files[entry]).subarray(symbol.start, symbol.end).toString()),
  ).toEqual(expect.arrayContaining(["r", "k", "v"]));
});

test("SFC checker errors participate in collect mode with original editor coordinates", async (t) => {
  const entry = "Diagnostic.vue";
  const source = wrap(
    "vue",
    `// 日本語 🧭
import {style} from 'stillwind/vue';const a=style('red'),b=style('green');let selected=a;if(Math.random())selected=b;const props=style(selected);`,
  );
  const { compile } = harness(t, { [entry]: source }, entry);
  const result = await compile({ diagnostics: "collect" });
  expect(result).toBeTruthy();
  expect(result!.code).toBe(null);
  const error = result!.diagnostics[0];
  expect(error.code).toBe("SW12004");
  expect(source.slice(error.startUtf16, error.endUtf16)).toBe("selected");
  expect(Buffer.from(source).subarray(error.start, error.end).toString()).toBe("selected");
  expect(error.startLine).toBe(2);
});
