import assert from "node:assert/strict";
import test from "node:test";

import { transform as compileAstro } from "@astrojs/compiler";
import {
  compileReactProject,
  NativeDesignSystemLoader,
  NativeProjectSession,
} from "@stillwind/native";
import { projectFrameworkSource } from "@stillwind/projection";
import { compileScript, parse as parseVue } from "@vue/compiler-sfc";
import { compile as compileSvelte } from "svelte/compiler";

import { compileModule } from "../src/compiler.ts";
import type { ProjectCompilerOutput } from "../src/protocol.ts";

const examples = {
  vue: `<script setup lang="ts">\nimport {style} from 'stillwind/vue';\nconst active = true, external = 'extra';\n</script>\n<template><div v-bind="style(active ? 'flex' : 'grid', {class: external})" /></template>`,
  svelte: `<script lang="ts">\nimport {style} from 'stillwind/svelte';\nconst active = true, external = 'extra';\n</script>\n<div {...style(active ? 'flex' : 'grid', {class: external})}></div>`,
  astro: `---\n// 🧭\nimport {style} from 'stillwind/astro';\nconst active = true, external = 'extra';\n---\n<div {...style(active ? 'flex' : 'grid', {class: external})}></div>`,
};

void test("host compiles raw SFCs and updates session CSS after edits", async () => {
  const loader = new NativeDesignSystemLoader();
  loader.provide(
    0,
    "/theme.css",
    "@utility flex {display:flex;} @utility grid {display:grid;} @stillwind-slot utilities;",
  );
  const session = new NativeProjectSession(loader, true);
  const host = {
    resolve: async () => {
      throw new Error("Unexpected resolution");
    },
    read: async () => {
      throw new Error("Unexpected read");
    },
    watch: () => {},
    warn: () => {},
  };
  for (const [extension, source] of Object.entries(examples)) {
    const id = `/src/Component.${extension}`;
    const output = await compileModule(
      session,
      () => {
        throw new Error("Unexpected projected checker query");
      },
      source,
      id,
      "/",
      host,
    );
    assert.ok(output?.code?.includes(`<${extension === "astro" ? "div" : "script"}`));
    assert.deepEqual(output.diagnostics, []);
    assert.match(session.moduleCss(output.path), /display:\s*flex/);
    const edited = await compileModule(
      session,
      () => {
        throw new Error("Unexpected projected checker query");
      },
      source.replace("'flex'", "'grid'"),
      id,
      "/",
      host,
    );
    assert.ok(edited?.code);
    assert.doesNotMatch(session.moduleCss(output.path), /display:\s*flex/);
    assert.match(session.moduleCss(output.path), /display:\s*grid/);
  }
});

void test("SFC output fails on residual macros or mismatched attribute context", async () => {
  const compile = (input: Awaited<ReturnType<typeof projectFrameworkSource>>) =>
    (
      compileReactProject(
        [input],
        "@utility flex {display:flex;} @utility grid {display:grid;} @stillwind-slot utilities;",
      ) as ProjectCompilerOutput
    ).modules[0]!;
  const input = await projectFrameworkSource({
    id: "/src/Bad.vue",
    path: "/src/Bad.vue",
    source: examples.vue,
  });
  const quoted = input.regions?.find((region) => region.attribute_quote === "double");
  assert.ok(quoted);
  quoted.attribute_quote = "single";
  const wrongQuote = compile(input);
  assert.equal(wrongQuote.code, null);
  assert.equal(wrongQuote.diagnostics[0]?.code, "SHIMPORT001");
  const leaked = await projectFrameworkSource({
    id: "/src/Bad.vue",
    path: "/src/Bad.vue",
    source: examples.vue.replace("const active", "const leaked = style; const active"),
  });
  const leftover = compile(leaked);
  assert.equal(leftover.code, null);
  assert.equal(leftover.diagnostics[0]?.code, "SHIMPORT001");
  const missingTarget = await projectFrameworkSource({
    id: "/src/Bad.vue",
    path: "/src/Bad.vue",
    source: examples.vue.replace(
      "import {style}",
      "import css from 'stillwind/module-css'; import {style}",
    ),
  });
  const noVirtualModule = compile(missingTarget);
  assert.equal(noVirtualModule.code, null);
  assert.equal(noVirtualModule.diagnostics[0]?.code, "SHIMPORT001");
  assert.match(noVirtualModule.diagnostics[0]!.message, /host-allocated virtual module/);
});

const extra = [
  ...Object.entries(examples).map(([extension, source]) => ({
    extension,
    name: "explicit module CSS",
    source: source.replace(
      "import {style}",
      "import css from 'stillwind/module-css'; console.log(css); import {style}",
    ),
    imports: 1,
  })),
  {
    extension: "vue",
    name: "shared normal-script import",
    source: `<script>import {style} from 'stillwind/vue';</script><script setup>const external='extra';</script><template><div v-bind="style('flex', {class: external})"/></template>`,
    imports: 1,
  },
  {
    extension: "svelte",
    name: "shared module-script import",
    source: `<script module>import {style} from 'stillwind/svelte';</script><script>const external='extra';</script><div {...style('flex', {class: external})}></div>`,
    imports: 1,
  },
  {
    extension: "vue",
    name: "single quoted attribute",
    source: `<script setup>import {style} from 'stillwind/vue'; const external='extra';</script><template><div v-bind='style("flex", {class: external})'/></template>`,
    imports: 1,
  },
  {
    extension: "vue",
    name: "attribute whitespace",
    source: `<script setup>import {style} from 'stillwind/vue'; const external='extra';</script><template><div v-bind="  style('flex', {class: external})  "/></template>`,
    imports: 1,
  },
  {
    extension: "vue",
    name: "two scripts",
    source: `<script>import {style as sm} from 'stillwind/vue'; export const a=sm('flex', {class: 'module'});</script><script setup>import {style} from 'stillwind/vue'; const b=style('grid', {class: 'instance'});</script><template><div v-bind="b"/></template>`,
    imports: 2,
  },
  {
    extension: "svelte",
    name: "two scripts",
    source: `<script module>import {style as sm} from 'stillwind/svelte'; export const a=sm('flex', {class: 'module'});</script><script>import {style} from 'stillwind/svelte'; const b=style('grid', {class: 'instance'});</script><div {...b}></div>`,
    imports: 2,
  },
  {
    extension: "astro",
    name: "client script",
    source: `<script>import {style} from 'stillwind/astro'; console.log(style('flex', {class:'client'}));</script><div>🧭</div>`,
    imports: 1,
  },
  {
    extension: "astro",
    name: "server and client scripts",
    source: `---\nimport {style} from 'stillwind/astro'; const a=style('flex', {class:'server'});\n---\n<div {...a}></div><script>import {style as sc} from 'stillwind/astro'; console.log(sc('grid', {class:'client'}));</script>`,
    imports: 2,
  },
];

for (const { extension, source, name, imports } of [
  ...Object.entries(examples).map(([extension, source]) => ({
    extension,
    source,
    name: "basic",
    imports: 1,
  })),
  ...extra,
]) {
  void test(`${extension} ${name}: converter, native edits and framework compilation agree`, async () => {
    const filename = `/src/Component.${extension}`;
    const projected = await projectFrameworkSource({ id: filename, path: filename, source });
    projected.stylesheet_imports = [
      "virtual:stillwind.css",
      "virtual:stillwind/carrier.css?revision=1",
    ];
    projected.module_css_import = "virtual:stillwind/module-css?test=1";
    assert.equal(projected.fw_code, source);
    assert.ok(projected.converter?.version);
    assert.ok(projected.import_realms?.length);
    const output = compileReactProject(
      [projected],
      "@utility flex {display:flex;} @utility grid {display:grid;} @stillwind-slot utilities;",
    ) as ProjectCompilerOutput;
    const module = output.modules[0]!;
    assert.deepEqual(module.diagnostics, []);
    assert.ok(module.code);
    const explicit = source.includes("stillwind/module-css");
    assert.equal(module.code.split('import "virtual:stillwind.css";').length, explicit ? 1 : 2);
    assert.equal(
      module.code.split('import "virtual:stillwind/carrier.css?revision=1";').length,
      explicit ? 1 : 2,
    );
    if (explicit)
      assert.match(module.code, /import css from "virtual:stillwind\/module-css\?test=1"/);
    assert.ok(module.code.includes(`stillwind/${extension}/runtime`), module.code);
    assert.ok(!module.code.includes("import {style}"), module.code);
    assert.ok(!module.code.includes("style(active"), module.code);
    assert.ok(!module.code.includes("__VLS_ctx"), module.code);
    assert.equal(module.code.match(/\/runtime/g)?.length, imports, module.code);
    if (extension === "vue") {
      const parsed = parseVue(module.code, { filename });
      assert.deepEqual(parsed.errors, []);
      compileScript(parsed.descriptor, { id: "fixture", inlineTemplate: true });
    } else if (extension === "svelte") {
      compileSvelte(module.code, { filename, generate: "client" });
    } else {
      const built = await compileAstro(module.code, { filename });
      assert.deepEqual(
        built.diagnostics.filter((diagnostic) => diagnostic.severity === 1),
        [],
      );
    }
  });
}
