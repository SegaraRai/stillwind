import { createRequire } from "node:module";
import path from "node:path";

import { compileProject, scanSource } from "@stillwind/native";
import type { prepareSource, PreparedSource } from "@stillwind/projection";
import { compile as compileSvelte } from "svelte/compiler";
import { afterAll, beforeAll, describe, expect, test, vi } from "vite-plus/test";

import { TypeScriptFactsService } from "../../src/type-facts.ts";

const projectionRequire = createRequire(import.meta.resolve("@stillwind/projection/package.json"));
const converterPath = projectionRequire.resolve("svelte2tsx");
const root = path.resolve(import.meta.dirname, "../../../integration/testss");
const filename = path.join(root, "Projection.svelte");
const macroImport = "import {style} from 'stillwind/svelte';";
const baseDeclaration = "const base=style('flex');";
const stylesheet = {
  content: "@utility flex {display:flex;} @utility grid {display:grid;} @stillwind-slot utilities;",
  name: "theme.css",
  uri: "untitled:theme.css",
};

function component(script: string, template: string, language = "ts") {
  return `<script${language === "ts" ? ' lang="ts"' : ""}>\n// 日本語 🧭\n${macroImport}${baseDeclaration}${script}\n</script>\n${template}`;
}

const expressionCases = [
  {
    name: "generic arrow functions use TypeScript syntax",
    script: "const identity=<T>(value:T)=>value;",
    template: '<div {...style(identity(base),"grid")}></div>',
  },
  {
    name: "const declarations remain visible to following template expressions",
    script: "",
    template: '{const chosen=base}<div {...style(chosen,"grid")}></div>',
  },
  {
    name: "let declarations preserve state types",
    script: "",
    template: '{let chosen=$state(base)}<div {...style(chosen,"grid")}></div>',
  },
  {
    name: "derived declarations preserve inferred types",
    script: "",
    template: '{const chosen=$derived(base)}<div {...style(chosen,"grid")}></div>',
  },
  {
    name: "attachments retain their macro calls",
    script: "",
    template: '<div {@attach ()=>{const chosen=style(base,"grid");console.log(chosen)}}></div>',
  },
  {
    name: "typed await bindings retain their annotations",
    script: "const pending=Promise.resolve(base);",
    template: '{#await pending then item:typeof base}<div {...style(item,"grid")}></div>{/await}',
  },
  {
    name: "destructured await bindings retain their annotations",
    script: "const pending=Promise.resolve({item:base});",
    template:
      '{#await pending then {item}:{item:typeof base}}<div {...style(item,"grid")}></div>{/await}',
  },
  {
    name: "await expressions retain finite identities",
    script: "const pending=Promise.resolve(base);",
    template: '<div {...style(await pending,"grid")}></div>',
  },
  {
    name: "typed snippets retain their parameter types",
    script: "",
    template:
      '{#snippet row(item:typeof base)}<div {...style(item,"grid")}></div>{/snippet}{@render row(base)}',
  },
  {
    name: "getter and setter bindings retain their macro calls",
    script: "let value='';",
    template:
      '<input bind:value={()=>value,(v)=>{const chosen=style(base,"grid");console.log(chosen);value=v}} />',
  },
];

for (const version of ["0.7.56", "0.7.61"]) {
  describe(`Svelte svelte2tsx ${version}`, () => {
    const alias = `svelte2tsx-${version.replaceAll(".", "-")}`;
    const convert = vi.fn(projectionRequire(alias).svelte2tsx);
    let prepare: typeof prepareSource;
    let facts: TypeScriptFactsService;
    const checkerTypes = [
      projectionRequire.resolve(`${alias}/svelte-shims-v4.d.ts`),
      projectionRequire.resolve(`${alias}/svelte-jsx-v4.d.ts`),
    ];

    beforeAll(async () => {
      vi.resetModules();
      vi.doMock(converterPath, () => ({ svelte2tsx: convert }));
      ({ prepareSource: prepare } = await import("@stillwind/projection"));
      facts = new TypeScriptFactsService(root);
    });

    afterAll(() => {
      facts?.close();
      vi.doUnmock(converterPath);
      vi.resetModules();
    });

    async function project(code: string) {
      const prepared = await prepare({
        moduleId: "Projection.svelte",
        resolverId: filename,
        checkerFileName: filename,
        projectionFileName: filename,
        document: { uri: "untitled:Projection.svelte", format: "svelte", code },
        syntax: { language: "ts", moduleKind: "module" },
      });
      if (prepared.kind !== "projected") {
        expect.fail("a Svelte document must produce a projection");
      }
      expect(convert).toHaveBeenLastCalledWith(
        code,
        expect.objectContaining({ filename, isTsFile: true, emitOnTemplateError: false }),
      );
      prepared.projection.converter.version = version;
      return prepared;
    }

    function collect(prepared: PreparedSource) {
      const summary = scanSource(prepared);
      const typeFacts = facts
        .collect(
          [
            {
              ...prepared,
              probes: summary.typeProbes,
              checkerSpans: summary.checkerSpans,
              checkerBindings: summary.checkerBindings,
              checkerTypes,
            },
          ],
          new Map([[prepared.moduleId, summary.assets]]),
        )
        .get(prepared.moduleId)!;
      return { summary, typeFacts };
    }

    function compile(prepared: PreparedSource) {
      const collected = collect(prepared);
      const output = compileProject([{ ...prepared, typeFacts: collected.typeFacts }], stylesheet);
      const module = output.modules[0];
      expect(module.diagnostics).toStrictEqual([]);
      if (module.code === null) {
        expect.fail("a compiled Svelte document must contain code");
      }
      compileSvelte(module.code, { filename, experimental: { async: true } });
      return { ...collected, module };
    }

    for (const { name, script, template } of expressionCases) {
      test(name, async () => {
        const prepared = await project(component(script, template));
        const { module } = compile(prepared);
        expect(module.composition).toHaveLength(1);
        const identity = module.composition[0].base;
        expect(identity.moduleId).toBe(prepared.moduleId);
        expect(
          Buffer.from(prepared.document.code).subarray(identity.start, identity.end).toString(),
        ).toBe("style('flex')");
        expect(module.css).toMatch(/display:\s*grid/);
      });
    }

    for (const language of ["ts", "js"]) {
      test(`${language}: generated helpers use global types beside authored helper calls`, async () => {
        const declaration =
          language === "ts"
            ? "function __sveltets_2_store_get<T>(value:T):T{return value;}"
            : "function __sveltets_2_store_get(value){return String(value);}";
        const authoredCall =
          language === "ts" ? "__sveltets_2_store_get(base)" : "__sveltets_2_store_get('authored')";
        const authoredUse =
          language === "ts"
            ? `<div {...style(${authoredCall},'grid')}></div>`
            : `<p>{${authoredCall}}</p>`;
        const prepared = await project(
          component(
            `import {writable} from 'svelte/store';const store=writable(base);${declaration}
const globalThis={__sveltets_2_store_get};
const __stillwindCheckerHelper\\u0030=0,__stillwindCheckerHelper1=0;`,
            `<div {...style($store,'grid')}></div>${authoredUse}`,
            language,
          ),
        );
        const { module, summary } = compile(prepared);
        expect(module.composition).toHaveLength(language === "ts" ? 2 : 1);
        expect(module.code).toContain(authoredCall);
        const bindings = summary.checkerBindings.filter(
          ({ name }) => name === "__sveltets_2_store_get",
        );
        expect(bindings).not.toHaveLength(0);
        const authoredStart = prepared.projection.code.indexOf(authoredCall);
        expect(authoredStart).toBeGreaterThanOrEqual(0);
        for (const binding of bindings) {
          expect(binding.projectedStart).not.toBe(authoredStart);
          expect(prepared.projection.code.slice(binding.projectedStart, binding.projectedEnd)).toBe(
            binding.name,
          );
        }
      });
    }

    test("nullable each loops use the global array helper", async () => {
      const prepared = await project(
        component(
          "const choices:typeof base[]|null|undefined=Math.random()?[base]:null;function __sveltets_2_ensureArray(value:unknown){return [0];}",
          '{#each choices as item}<div {...style(item,"grid")}></div>{/each}',
        ),
      );
      const { module } = compile(prepared);
      expect(module.composition).toHaveLength(1);
    });

    test("bindable props retain identities and bind the generated bindings helper", async () => {
      const prepared = await project(
        component(
          "let {choice=$bindable(base)}:{choice?:typeof base}=$props();function __sveltets_$$bindings(){return false;}",
          '<div {...style(choice,"grid")}></div>',
        ),
      );
      const { module, summary } = compile(prepared);
      expect(module.composition).toHaveLength(1);
      expect(summary.checkerBindings.map(({ name }) => name)).toContain("__sveltets_$$bindings");
    });

    test("legacy slots keep their generated local helper binding", async () => {
      const prepared = await project(
        component("", '<slot item={base}></slot><div {...style(base,"grid")}></div>'),
      );
      const { module, summary } = compile(prepared);
      expect(module.composition).toHaveLength(1);
      expect(summary.checkerBindings.map(({ name }) => name)).toContain(
        "__sveltets_2_createCreateSlot",
      );
      expect(summary.checkerBindings.map(({ name }) => name)).not.toContain(
        "__sveltets_createSlot",
      );
    });

    for (const moduleAttribute of ['context="module"', "module"]) {
      test(`${moduleAttribute}: module and instance scripts retain their import realms`, async () => {
        const prepared = await project(
          `<script ${moduleAttribute} lang="ts">import {style as outerStyle} from 'stillwind/svelte';export const outer=outerStyle('flex');</script><script lang="ts">${macroImport}const inner=style(outer,'grid');</script><div {...inner}></div>`,
        );
        const { module } = compile(prepared);
        expect(module.composition).toHaveLength(1);
        expect(module.code).toContain(`<script ${moduleAttribute} lang="ts">`);
        expect(module.css).toMatch(/display:\s*flex/);
        expect(module.css).toMatch(/display:\s*grid/);
      });
    }

    test("CRLF and Unicode retain original declaration coordinates", async () => {
      const prepared = await project(
        component("", '<div {...style(base,"grid")}></div>').replaceAll("\n", "\r\n"),
      );
      const { module } = compile(prepared);
      const identity = module.composition[0].base;
      expect(
        Buffer.from(prepared.document.code).subarray(identity.start, identity.end).toString(),
      ).toBe("style('flex')");
      expect(module.code).toContain("// 日本語 🧭\r\n");
    });

    test("an unknown await annotation rejects erased identity at its original use", async () => {
      const code = component(
        "const pending=Promise.resolve(base);",
        '{#await pending then item:unknown}<div {...style(item,"grid")}></div>{/await}',
      );
      const prepared = await project(code);
      const start = Buffer.byteLength(code.slice(0, code.indexOf('style(item,"grid")') + 6));
      expect(() => collect(prepared)).toThrow(
        expect.objectContaining({
          code: "SHTS003",
          moduleId: prepared.moduleId,
          start,
          end: start + 4,
        }),
      );
    });
  });
}
