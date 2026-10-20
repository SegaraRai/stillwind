import { createRequire } from "node:module";
import path from "node:path";

import { compileProject, scanSource } from "@stillwind/native";
import type { prepareSource, PreparedSource } from "@stillwind/projection";
import { afterAll, beforeAll, describe, expect, test, vi } from "vite-plus/test";
import * as core302 from "vue-language-core-3-0-2";
import * as core306 from "vue-language-core-3-0-6";
import * as core310 from "vue-language-core-3-1-0";
import * as core316 from "vue-language-core-3-1-6";
import * as core320 from "vue-language-core-3-2-0";
import * as core3312 from "vue-language-core-3-3-12";

import { TypeScriptFactsService } from "../../src/semantics/index.ts";

const versions = [
  {
    version: "3.0.2",
    core: core302,
    exposure: /setup\(\)\s*\{\s*return\s*\{/,
  },
  {
    version: "3.0.6",
    core: core306,
    exposure: /setup:\s*\(\)\s*=>\s*\(\{/,
  },
  {
    version: "3.1.0",
    core: core310,
    exposure: /type __VLS_Bindings = __VLS_ProxyRefs</,
  },
  {
    version: "3.1.6",
    core: core316,
    exposure: /type __VLS_SetupExposed = __VLS_ProxyRefs</,
  },
  {
    version: "3.2.0",
    core: core320,
    exposure: /type __VLS_SetupExposed = import\('vue'\)\.ShallowUnwrapRef</,
  },
  {
    version: "3.3.12",
    core: core3312,
    exposure: /__VLS_unwrap\(style,/,
  },
];

const root = path.resolve(import.meta.dirname, "../../../integration/testss");
const filename = path.join(root, "Projection.vue");
const corePath = createRequire(import.meta.resolve("@stillwind/projection/package.json")).resolve(
  "@vue/language-core",
);
const macroImport = "import {style} from 'stillwind/vue';";
const stylesheet = {
  content: "@utility flex {display:flex;} @utility grid {display:grid;} @stillwind-slot utilities;",
  name: "theme.css",
  uri: "untitled:theme.css",
};

function component(script: string, template: string, language = "ts") {
  return `<script setup lang="${language}">\n${script}\n</script>\n<template>${template}</template>`;
}

const expressionCases = [
  {
    name: "context receivers and conditional utilities",
    script: "const active=true, external='extra';",
    template: "<div v-bind=\"style(active ? 'flex' : 'grid',{class:external})\"/>",
    retained: ["active", "external"],
  },
  {
    name: "expanded shorthand properties",
    script: "const external='extra';",
    template: "<div v-bind=\"style('flex',{class:String({external})})\"/>",
    retained: ["String({external})"],
  },
  {
    name: "nested shorthand properties",
    script: "const external='extra';",
    template: "<div v-bind=\"style('flex',{class:JSON.stringify({outer:{external}})})\"/>",
    retained: ["JSON.stringify({outer:{external}})"],
  },
  {
    name: "authored value access beside inserted value access",
    script: "const state={value:true};",
    template: "<div v-bind=\"style(state.value ? 'flex' : 'grid')\"/>",
    retained: ["state.value"],
  },
  {
    name: "repeated authored value access",
    script: "const state={value:{value:true}};",
    template: "<div v-bind=\"style(state.value.value ? 'flex' : 'grid')\"/>",
    retained: ["state.value.value"],
  },
  {
    name: "optional authored value access",
    script: "const state={value:true};",
    template: "<div v-bind=\"style(state?.value ? 'flex' : 'grid')\"/>",
    retained: ["state?.value"],
  },
  {
    name: "computed authored access",
    script: "const state={value:true}, key='value';",
    template: "<div v-bind=\"style(state[key] ? 'flex' : 'grid')\"/>",
    retained: ["state[key]"],
  },
  {
    name: "authored context and dollars receivers",
    script: "const __VLS_ctx={active:true}, __VLS_dollars={label:'authored'};",
    template:
      "<div v-bind=\"style(__VLS_ctx.active ? 'flex' : 'grid',{'--label':__VLS_dollars.label})\"/>",
    retained: ["__VLS_ctx.active", "__VLS_dollars.label"],
  },
  {
    name: "framework dollars receiver",
    script: "",
    template: "<div v-bind=\"style('flex',{class:$attrs.class})\"/>",
    retained: ["$attrs.class"],
  },
  {
    name: "template loop shadowing",
    script: "const items=[{style:()=>({})}];",
    template: '<div v-for="({style},i) in items" v-bind="style()"/><div v-bind="style(\'flex\')"/>',
    retained: ['v-bind="style()"'],
  },
  {
    name: "slot shadowing",
    script: "",
    template:
      '<Component v-slot="{style}"><div v-bind="style()"/></Component><div v-bind="style(\'flex\')"/>',
    retained: ['v-bind="style()"'],
  },
  {
    name: "nested completion lists in event closures",
    script: "const active=true, other=false;",
    template: "<button v-if=\"active\" @click=\"other ? style('flex') : style('grid')\"/>",
    retained: ["other"],
  },
  {
    name: "authored helper calls",
    script: "function __VLS_unwrap(x,y){return x;} function __VLS_withDotValue(x,y){return x;}",
    template:
      "<div v-bind=\"style('flex',{class:__VLS_unwrap('authored')+__VLS_withDotValue('authored')})\"/>",
    retained: ["__VLS_unwrap('authored')", "__VLS_withDotValue('authored')"],
  },
  {
    name: "single quoted directive expressions",
    script: "const external='extra';",
    template: "<div v-bind='style(\"flex\",{class:external})'/>",
    retained: ["external"],
  },
  {
    name: "Unicode and CRLF source positions",
    script: "// 日本語 🧭\r\nconst 状態=true, 名前='外部';",
    template: "<div v-bind=\"style(状態 ? 'flex' : 'grid',{class:名前})\"/>",
    retained: ["状態", "名前"],
  },
];

const typeCases = [
  {
    name: "qualified type queries",
    expression: "String({} as typeof state)",
  },
  {
    name: "type assertions around authored access",
    expression: "String((state as {value:boolean}).value)",
  },
  {
    name: "generated constructor parentheses",
    expression: "String(new Factory())",
  },
  {
    name: "authored constructor parentheses",
    expression: "String(new (Factory)())",
  },
  {
    name: "imported tagged templates",
    expression: "tag`authored`",
  },
];

for (const { version, core, exposure } of versions) {
  describe(`Vue language-core ${version}`, () => {
    let prepare: typeof prepareSource;
    let facts: TypeScriptFactsService;

    beforeAll(async () => {
      vi.resetModules();
      vi.doMock(corePath, () => ({
        ...core,
        generateGlobalTypes: "generateGlobalTypes" in core ? core.generateGlobalTypes : undefined,
      }));
      ({ prepareSource: prepare } = await import("@stillwind/projection"));
      facts = new TypeScriptFactsService({
        root,
        project: {
          kind: "isolated",
        },
      });
    });

    afterAll(() => {
      facts?.close();
      vi.doUnmock(corePath);
      vi.resetModules();
    });

    async function project(code: string) {
      const prepared = await prepare({
        moduleId: "Projection.vue",
        resolverId: filename,
        checkerFileName: filename,
        projectionFileName: filename,
        document: { uri: "untitled:Projection.vue", format: "vue", code },
        syntax: { language: "ts", moduleKind: "module" },
      });
      if (prepared.kind !== "projected") {
        expect.fail("a Vue document must produce a projection");
      }
      prepared.projection.converter.version = version;
      return prepared;
    }

    function compile(prepared: PreparedSource) {
      const scan = scanSource(prepared);
      if (scan.kind !== "ready") {
        expect.fail(JSON.stringify(scan.batch.reports));
      }
      const summary = scan.summary;
      const otherId = path.join(root, "other.ts");
      const other: PreparedSource = {
        kind: "direct",
        moduleId: "other.ts",
        resolverId: otherId,
        checkerFileName: otherId,
        projectionFileName: otherId,
        document: {
          uri: "untitled:other.ts",
          format: "script",
          code: "export class Factory {} export function tag(strings:TemplateStringsArray){return strings[0];}",
        },
        syntax: {
          language: "ts",
          moduleKind: "module",
        },
      };
      const typeFacts = facts
        .collect(
          [
            {
              ...prepared,
              probes: summary.typeProbes,
              checkerSpans: summary.checkerSpans,
              checkerBindings: summary.checkerBindings,
              resolutions: summary.moduleSpecifiers
                .filter((specifier) => specifier.source === "./other.ts")
                .map(({ start, end }) => ({
                  start,
                  end,
                  id: otherId,
                })),
            },
            {
              ...other,
              probes: [],
            },
          ],
          new Map([[prepared.moduleId, summary.assets]]),
        )
        .get(prepared.moduleId)!;
      const output = compileProject(
        [
          {
            ...prepared,
            typeFacts,
          },
        ],
        stylesheet,
      );
      expect(output.modules[0].diagnostics).toStrictEqual([]);
      return {
        summary,
        typeFacts,
        module: output.modules[0],
      };
    }

    test("the pinned converter emits its expected exposure form", async () => {
      const prepared = await project(
        component(`${macroImport} const base=style('flex');`, '<div v-bind="style(base)"/>'),
      );
      expect(prepared.projection.code).toMatch(exposure);
      const { module } = compile(prepared);
      expect(module.composition).toHaveLength(1);
    });

    for (const language of ["ts", "js"]) {
      for (const { name, script, template, retained } of expressionCases) {
        test(`${language}: ${name}`, async () => {
          const code = component(`${macroImport} ${script}`, template, language);
          const prepared = await project(code);
          const { module } = compile(prepared);
          expect(module.css).toMatch(/display:\s*flex/);
          for (const expression of retained) {
            expect(module.code).toContain(expression);
          }
        });
      }

      for (const reference of ["ref", "shallowRef"]) {
        for (const narrowed of [false, true]) {
          test(`${language}: ${reference} composition${narrowed ? " after narrowing" : ""}`, async () => {
            const prepared = await project(
              component(
                `${macroImport} import {${reference}} from 'vue';\n// 日本語 🧭\nconst base=style('flex'); const chosen=${reference}(base);`,
                `<div ${narrowed ? 'v-if="chosen" ' : ""}v-bind="style(chosen,'grid')"/>`,
                language,
              ),
            );
            const { module, typeFacts } = compile(prepared);
            expect(module.composition).toHaveLength(1);
            const identity = module.composition[0].base;
            expect(identity.moduleId).toBe(prepared.moduleId);
            expect(
              Buffer.from(prepared.document.code).subarray(identity.start, identity.end).toString(),
            ).toBe("style('flex')");
            const chosen = typeFacts.find(
              (fact) =>
                Buffer.from(prepared.document.code).subarray(fact.start, fact.end).toString() ===
                "chosen",
            );
            expect(chosen?.value).toStrictEqual({
              kind: "styles",
              value: {
                kind: "style",
                candidates: {
                  kind: "finite",
                  values: [identity],
                },
              },
            });
            expect(module.css).toMatch(/display:\s*grid/);
          });
        }
      }

      test(`${language}: macro import aliases`, async () => {
        const prepared = await project(
          component(
            "import {style as sw} from 'stillwind/vue'; const active=true;",
            "<div v-bind=\"sw(active ? 'flex' : 'grid')\"/>",
            language,
          ),
        );
        const { module } = compile(prepared);
        expect(module.css).toMatch(/display:\s*flex/);
        expect(module.css).toMatch(/display:\s*grid/);
      });
    }

    for (const { name, expression } of typeCases) {
      test(`ts: ${name}`, async () => {
        const prepared = await project(
          component(
            `${macroImport} import {Factory,tag} from './other.ts'; const state={value:true};`,
            `<div v-bind="style('flex',{class:${expression}})"/>`,
          ),
        );
        const { module } = compile(prepared);
        expect(module.code).toContain(expression);
      });
    }

    test("template props and loops retain finite style candidates", async () => {
      const prepared = await project(
        component(
          `${macroImport} const {enabled}=defineProps<{enabled:boolean}>();const a=style('flex'),b=style('grid');const choices=[a,b];`,
          '<div v-bind="style(enabled?a:b)"/><div v-for="item in choices" v-bind="style(item)"/>',
        ),
      );
      const { module } = compile(prepared);
      expect(module.composition).toHaveLength(4);
      expect(
        new Set(
          module.composition.map(({ base }) =>
            Buffer.from(prepared.document.code).subarray(base.start, base.end).toString(),
          ),
        ),
      ).toStrictEqual(new Set(["style('flex')", "style('grid')"]));
    });

    test("unwrapped refs, keyframes and transitions retain their asset kinds", async () => {
      const prepared = await project(
        component(
          `${macroImport} const r=style.ref();const k=style.keyframes({from:'flex',to:'grid'});const v=style.viewTransition({old:'flex'});`,
          "<div v-bind=\"style(r,r.asAnchor(),k(''),v)\"/>",
        ),
      );
      const { module, typeFacts } = compile(prepared);
      const kinds = new Set(typeFacts.flatMap(({ value }) => (value ? [value.kind] : [])));
      expect(kinds).toStrictEqual(new Set(["refs", "keyframes", "viewTransitions"]));
      expect(
        module.symbols
          .filter(({ sourceId }) => sourceId === prepared.moduleId)
          .map(({ start, end }) =>
            Buffer.from(prepared.document.code).subarray(start, end).toString(),
          ),
      ).toEqual(expect.arrayContaining(["r", "k", "v"]));
    });

    test("copied component options retain one original producer identity", async () => {
      const code = `<script lang="ts">${macroImport} export default {setup(){return {x:style('flex')}}};</script><script setup lang="ts">const b='grid';</script><template><div v-bind="x"/></template>`;
      const prepared = await project(code);
      const { summary, module } = compile(prepared);
      expect(summary.assets).toHaveLength(1);
      const asset = summary.assets[0];
      expect(Buffer.from(code).subarray(asset.callStart, asset.callEnd).toString()).toBe(
        "style('flex')",
      );
      const producerSpans = summary.checkerSpans.filter((span) => span.role === "producer");
      expect(producerSpans).toHaveLength(
        version.startsWith("3.0.") || version.startsWith("3.1.") ? 2 : 1,
      );
      expect(module.css).toMatch(/display:\s*flex/);
    });

    test("normal and setup scripts preserve independent macro imports", async () => {
      const prepared = await project(
        `<script lang="ts">import {style as a} from 'stillwind/vue';export const x=a('flex');</script><script setup lang="ts">import {style as b} from 'stillwind/vue';const y=b('grid');</script><template><div v-bind="a('flex')"/><div v-bind="b('grid')"/></template>`,
      );
      const { module } = compile(prepared);
      expect(module.css).toMatch(/display:\s*flex/);
      expect(module.css).toMatch(/display:\s*grid/);
      expect(module.code).toContain('<script lang="ts">');
      expect(module.code).toContain('<script setup lang="ts">');
    });

    test("generated checker helpers preserve identities beside authored helper names", async () => {
      const prepared = await project(
        component(
          `${macroImport} import {ref} from 'vue'; const base=style('flex');const chosen=ref(base);
function __VLS_unwrap(x:unknown,y?:unknown){return String(x);}
function __VLS_withDotValue(x:unknown,y?:unknown){}
const globalThis={__VLS_unwrap,__VLS_withDotValue};
const __stillwindCheckerHelper\\u0030=0, __stillwindCheckerHelper1=0;`,
          '<div v-if="chosen" v-bind="style(chosen)"/>',
        ),
      );
      const { module } = compile(prepared);
      expect(module.composition).toHaveLength(1);
      expect(module.code).toContain("const globalThis={__VLS_unwrap,__VLS_withDotValue}");
    });

    const unconsumed = [
      {
        name: "authored script read",
        script: "const leaked=style; const x=style('flex');",
        template: '<div v-bind="x"/>',
      },
      {
        name: "authored template read",
        script: "",
        template: "<div>{{style}}</div><div v-bind=\"style('flex')\"/>",
      },
      {
        name: "authored exposure type alias",
        script:
          "type __VLS_SetupExposed=import('vue').ShallowUnwrapRef<{style:typeof style}>; const x=style('flex');",
        template: '<div v-bind="x"/>',
      },
      {
        name: "authored completion array",
        script: "const x=style('flex');\n// @ts-ignore\n[style];\nconst __VLS_export={};",
        template: '<div v-bind="x"/>',
      },
    ];
    for (const { name, script, template } of unconsumed) {
      test(`${name} prevents macro import removal`, async () => {
        const prepared = await project(component(`${macroImport} ${script}`, template));
        const output = compileProject([prepared], stylesheet);
        expect(output.modules[0].diagnostics.map(({ code }) => code)).toContain("SW10005");
      });
    }
  });
}
