import assert from "node:assert/strict";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { transform as compileAstro } from "@astrojs/compiler";
import { TraceMap, originalPositionFor } from "@jridgewell/trace-mapping";
import { compileProject, NativeDesignSystemLoader, NativeProjectSession } from "@stillwind/native";
import { checkerEnvironment, projectFrameworkSource } from "@stillwind/projection";
import { compileScript, parse as parseVue } from "@vue/compiler-sfc";
import { compile as compileSvelte } from "svelte/compiler";

import { compileModule, fileModuleDescriptor, fileSourceKind } from "../src/compiler.ts";
import type { ProjectCompilerOutput } from "../src/protocol.ts";
import { moduleSourceMap } from "../src/source-map.ts";
import { TypeScriptFactsService } from "../src/type-facts.ts";

const cssInput = (content: string) => ({
  content,
  name: "root.css",
  uri: "untitled:root.css",
  sourceMapSource: "untitled:root.css",
});

const examples = {
  vue: `<script setup lang="ts">\nimport {style} from 'stillwind/vue';\nconst active = true, external = 'extra';\n</script>\n<template><div v-bind="style(active ? 'flex' : 'grid', {class: external})" /></template>`,
  svelte: `<script lang="ts">\nimport {style} from 'stillwind/svelte';\nconst active = true, external = 'extra';\n</script>\n<div {...style(active ? 'flex' : 'grid', {class: external})}></div>`,
  astro: `---\n// 🧭\nimport {style} from 'stillwind/astro';\nconst active = true, external = 'extra';\n---\n<div {...style(active ? 'flex' : 'grid', {class: external})}></div>`,
};

void test("filename inference describes input format without selecting an output adapter", () => {
  for (const [name, language, sourceFormat] of [
    ["Component.vue", "js", "vue"],
    ["Component.svelte", "js", "svelte"],
    ["Component.astro", "js", "astro"],
    ["Component.TSX?raw#fragment", "tsx", "jsx"],
    ["Component.jsx", "jsx", "jsx"],
    ["module.ts", "ts", "script"],
    ["module.mts", "ts", "script"],
    ["module.cts", "ts", "script"],
    ["module.js", "js", "script"],
    ["module.mjs", "js", "script"],
    ["module.cjs", "js", "script"],
    ["module.d.ts", "dts", "script"],
    ["module.d.mts", "dts", "script"],
    ["module.d.cts", "dts", "script"],
  ]) {
    assert.deepEqual(fileSourceKind(name), {
      language,
      moduleKind: "unambiguous",
      sourceFormat,
      framework: undefined,
    });
  }
});

void test("Vue output adapter leaves JSX and unspecified input formats unprojected", async () => {
  const source = `import {style} from 'stillwind/vue'; const view = <div {...style('flex')} />;`;
  const input = {
    resolverId: "/src/Component.tsx",
    ...fileModuleDescriptor(path.resolve("/src"), path.resolve("/src/Component.tsx")),
    framework: "vue" as const,
    source,
  };
  for (const sourceFormat of ["jsx", undefined] as const) {
    const projected = await projectFrameworkSource({ ...input, sourceFormat });
    assert.equal(projected.source, source);
    assert.equal(projected.sourceFormat, sourceFormat);
    assert.equal(projected.framework, "vue");
    assert.equal(projected.fwCode, undefined);
    assert.equal(projected.converter, undefined);
    assert.deepEqual(checkerEnvironment(projected), { checkerTypes: [] });
  }
});

void test("SFC source formats select converters and preserve explicit output adapters", async () => {
  const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../integration/test");
  for (const [extension, source] of Object.entries(examples)) {
    const filename = path.join(root, `Component.${extension}`);
    const input = {
      resolverId: filename,
      ...fileModuleDescriptor(root, filename),
      source,
      map: "upstream map",
    };
    for (const framework of [undefined, "react"] as const) {
      const projected = await projectFrameworkSource({ ...input, framework });
      assert.equal(projected.sourceFormat, extension);
      assert.equal(projected.framework, framework);
      assert.equal(projected.fwCode, source);
      assert.ok(projected.converter?.version);
      assert.ok(projected.fw2JsMap);
      assert.equal(projected.fwMap, input.map);
      assert.equal(projected.map, undefined);
      const environment = checkerEnvironment(projected);
      assert.equal(
        environment.checkerTypes.length,
        extension === "svelte" ? 2 : extension === "astro" ? 1 : 0,
      );
      assert.deepEqual(await projectFrameworkSource(projected), projected);
    }
    assert.equal(input.source, source);
    assert.equal(input.map, "upstream map");
  }
});

void test("checker declarations follow projected source format instead of output adapter", () => {
  const input = {
    resolverId: "/src/Component.tsx",
    ...fileModuleDescriptor(path.resolve("/src"), path.resolve("/src/Component.tsx")),
    source: "const view = <div />;",
    fwCode: "original buffer",
  };
  for (const framework of ["svelte", "astro"] as const) {
    assert.deepEqual(checkerEnvironment({ ...input, framework }), { checkerTypes: [] });
  }
});

void test("host compiles raw SFCs and updates session CSS after edits", async (t) => {
  const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../integration/test");
  const facts = new TypeScriptFactsService(root);
  t.after(() => facts.close());
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
    describe: (id) => ({
      ...fileModuleDescriptor(root, id),
      uri: `untitled:${encodeURIComponent(id)}`,
    }),
    watch: () => {},
    warn: () => {},
  };
  for (const [extension, source] of Object.entries(examples)) {
    const id = path.join(root, `Component.${extension}`);
    const output = await compileModule(session, () => facts, source, id, root, host);
    assert.ok(output?.code?.includes(`<${extension === "astro" ? "div" : "script"}`));
    assert.deepEqual(output.diagnostics, []);
    assert.match(session.moduleCss(output.moduleId), /display:\s*flex/);
    const edited = await compileModule(
      session,
      () => facts,
      source.replace("'flex'", "'grid'"),
      id,
      root,
      host,
    );
    assert.ok(edited?.code);
    assert.doesNotMatch(session.moduleCss(output.moduleId), /display:\s*flex/);
    assert.match(session.moduleCss(output.moduleId), /display:\s*grid/);
  }
});

void test("SFC output fails on residual macros or mismatched attribute context", async () => {
  const compile = (input: Awaited<ReturnType<typeof projectFrameworkSource>>) =>
    (
      compileProject(
        [input],
        cssInput(
          "@utility flex {display:flex;} @utility grid {display:grid;} @stillwind-slot utilities;",
        ),
      ) as ProjectCompilerOutput
    ).modules[0]!;
  const input = await projectFrameworkSource({
    resolverId: "/src/Bad.vue",
    checkerFileName: "/src/Bad.vue",
    projectionFileName: "/src/Bad.vue",
    moduleId: "/src/Bad.vue",
    ...fileSourceKind("/src/Bad.vue"),
    uri: "untitled:Bad.vue",
    source: examples.vue,
  });
  const quoted = input.regions?.find((region) => region.attributeQuote === "double");
  assert.ok(quoted);
  quoted.attributeQuote = "single";
  const wrongQuote = compile(input);
  assert.equal(wrongQuote.code, null);
  assert.equal(wrongQuote.diagnostics[0]?.code, "SW11008");
  const leaked = await projectFrameworkSource({
    resolverId: "/src/Bad.vue",
    checkerFileName: "/src/Bad.vue",
    projectionFileName: "/src/Bad.vue",
    moduleId: "/src/Bad.vue",
    ...fileSourceKind("/src/Bad.vue"),
    uri: "untitled:Bad.vue",
    source: examples.vue.replace("const active", "const leaked = style; const active"),
  });
  const leftover = compile(leaked);
  assert.equal(leftover.code, null);
  assert.equal(leftover.diagnostics[0]?.code, "SW11010");
  const missingTarget = await projectFrameworkSource({
    resolverId: "/src/Bad.vue",
    checkerFileName: "/src/Bad.vue",
    projectionFileName: "/src/Bad.vue",
    moduleId: "/src/Bad.vue",
    ...fileSourceKind("/src/Bad.vue"),
    uri: "untitled:Bad.vue",
    source: examples.vue.replace(
      "import {style}",
      "import css from 'stillwind/module-css'; import {style}",
    ),
  });
  const noVirtualModule = compile(missingTarget);
  assert.equal(noVirtualModule.code, null);
  assert.equal(noVirtualModule.diagnostics[0]?.code, "SW11009");
  assert.match(noVirtualModule.diagnostics[0]!.message, /host-allocated virtual module/);
});

const extra = [
  ...Object.entries(examples).map(([extension, source]) => ({
    extension,
    name: "local composition",
    source: source
      .replace("const active", "const base=style('flex'); const active")
      .replace("style(active", "style(base, active"),
    imports: 1,
  })),
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
    const projected = await projectFrameworkSource({
      resolverId: filename,
      checkerFileName: filename,
      projectionFileName: filename,
      moduleId: filename,
      ...fileSourceKind(filename),
      uri: `untitled:${filename}`,
      source,
    });
    projected.stylesheetImports = [
      "virtual:stillwind.css",
      "virtual:stillwind/carrier.css?revision=1",
    ];
    projected.moduleCssImport = "virtual:stillwind/module-css?test=1";
    assert.equal(projected.fwCode, source);
    assert.ok(projected.converter?.version);
    assert.ok(projected.importRealms?.length);
    const output = compileProject(
      [projected],
      cssInput(
        "@utility flex {display:flex;} @utility grid {display:grid;} @stillwind-slot utilities;",
      ),
    ) as ProjectCompilerOutput;
    const module = output.modules[0]!;
    assert.deepEqual(module.diagnostics, []);
    if (source.includes("const base=style")) {
      assert.equal(module.composition.length, 1);
      assert.equal(
        Buffer.from(source)
          .subarray(module.composition[0].base.start, module.composition[0].base.end)
          .toString("utf8"),
        "style('flex')",
      );
    }
    assert.ok(module.code);
    const map = new TraceMap(
      moduleSourceMap(source, module.code, filename, module.mappings, [], ""),
    );
    for (const expression of module.mappings.flatMap((mapping) => mapping.expressions)) {
      const generated = module.code.slice(expression.generatedStart, expression.generatedEnd);
      const original = source.slice(expression.originalStart, expression.originalEnd);
      if (expression.exact) {
        assert.equal(generated, original);
      }
      const origin = originalPositionFor(map, {
        line: module.code.slice(0, expression.generatedStart).split("\n").length,
        column:
          expression.generatedStart -
          module.code.lastIndexOf("\n", expression.generatedStart - 1) -
          1,
      });
      assert.equal(origin.line, source.slice(0, expression.originalStart).split("\n").length);
      assert.equal(
        origin.column,
        expression.originalStart - source.lastIndexOf("\n", expression.originalStart - 1) - 1,
      );
    }
    const explicit = source.includes("stillwind/module-css");
    assert.equal(module.code.split('import "virtual:stillwind.css";').length, explicit ? 1 : 2);
    assert.equal(
      module.code.split('import "virtual:stillwind/carrier.css?revision=1";').length,
      explicit ? 1 : 2,
    );
    if (explicit) {
      assert.match(module.code, /import css from "virtual:stillwind\/module-css\?test=1"/);
    }
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
