import path from "node:path";
import { fileURLToPath } from "node:url";

import { transform as compileAstro } from "@astrojs/compiler";
import { originalPositionFor, TraceMap } from "@jridgewell/trace-mapping";
import {
  compileProject,
  NativeDesignSystemLoader,
  NativeProjectSession,
  type ProjectBindingOutput,
} from "@stillwind/native";
import { checkerEnvironment, prepareSource, type PreparedSource } from "@stillwind/projection";
import { compileScript, parse as parseVue } from "@vue/compiler-sfc";
import { compile as compileSvelte } from "svelte/compiler";
import { expect, test } from "vite-plus/test";

import {
  compileModule,
  fileModuleDescriptor,
  fileSourceKind,
  moduleSourceMap,
} from "../src/compilation/index.ts";
import { TypeScriptFactsService } from "../src/semantics/index.ts";

const cssInput = (content: string) => ({
  content,
  name: "root.css",
  uri: "untitled:root.css",
  sourceMapSource: "untitled:root.css",
});

const examples = {
  vue: `<script setup lang="ts">\nimport { style } from "stillwind/vue";\nconst active = true, external = "extra";\n</script>\n<template><div v-bind="style(active ? 'flex' : 'grid', { class: external })" /></template>`,
  svelte: `<script lang="ts">\nimport { style } from "stillwind/svelte";\nconst active = true, external = "extra";\n</script>\n<div {...style(active ? "flex" : "grid", { class: external })}></div>`,
  astro: `---\n// 🧭\nimport { style } from "stillwind/astro";\nconst active = true, external = "extra";\n---\n<div {...style(active ? "flex" : "grid", { class: external })}></div>`,
};

test("filename inference describes input format without selecting an output adapter", () => {
  for (const [name, language, format] of [
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
    expect(fileSourceKind(name)).toStrictEqual({
      syntax: { language, moduleKind: "unambiguous" },
      format,
    });
  }
});

test("Vue output adapter leaves JSX and script input formats direct", async () => {
  const source = 'import { style } from "stillwind/vue"; const view = <div {...style("flex")} />;';
  const descriptor = fileModuleDescriptor(path.resolve("/src"), path.resolve("/src/Component.tsx"));
  for (const format of ["jsx", "script"] as const) {
    const prepared = await prepareSource({
      resolverId: "/src/Component.tsx",
      ...descriptor,
      framework: "vue",
      document: {
        ...descriptor.document,
        code: source,
        format,
      },
    });
    expect(prepared.kind).toBe("direct");
    expect(prepared.document.code).toBe(source);
    expect(prepared.document.format).toBe(format);
    expect(prepared.framework).toBe("vue");
    expect(checkerEnvironment(prepared)).toStrictEqual({ checkerTypes: [] });
  }
});

test("SFC source formats select converters and preserve explicit output adapters", async () => {
  const root = path.resolve(
    path.dirname(fileURLToPath(import.meta.url)),
    "../../integration/testss",
  );
  for (const [extension, source] of Object.entries(examples)) {
    const filename = path.join(root, `Component.${extension}`);
    const descriptor = fileModuleDescriptor(root, filename);
    const input = {
      resolverId: filename,
      ...descriptor,
      document: { ...descriptor.document, code: source, upstreamMap: "upstream map" },
    };
    for (const framework of [undefined, "react"] as const) {
      const projected = await prepareSource({ ...input, framework });
      expect(projected.kind).toBe("projected");
      if (projected.kind !== "projected") {
        expect.fail("expected a projected source");
      }
      expect(projected.document.format).toBe(extension);
      expect(projected.framework).toBe(framework);
      expect(projected.document.code).toBe(source);
      expect(projected.projection.converter.version).toBeTruthy();
      expect(projected.projection.map).toBeTruthy();
      expect(projected.document.upstreamMap).toBe(input.document.upstreamMap);
      const environment = checkerEnvironment(projected);
      expect(environment.checkerTypes.length).toBe(
        extension === "svelte" ? 2 : extension === "astro" ? 1 : 0,
      );
      expect(projected.document).toStrictEqual({
        ...input.document,
        sourceMapSource: input.document.uri,
      });
      expect(input.document.sourceMapSource).toBe(undefined);
    }
    expect(input.document.code).toBe(source);
    expect(input.document.upstreamMap).toBe("upstream map");
  }
});

test("checker declarations follow prepared kind instead of output adapter", async () => {
  const descriptor = fileModuleDescriptor(path.resolve("/src"), path.resolve("/src/Component.tsx"));
  for (const framework of ["svelte", "astro"] as const) {
    const prepared = await prepareSource({
      resolverId: "/src/Component.tsx",
      ...descriptor,
      document: {
        ...descriptor.document,
        code: "const view = <div />;",
      },
      framework,
    });
    expect(prepared.kind).toBe("direct");
    expect(checkerEnvironment(prepared)).toStrictEqual({ checkerTypes: [] });
  }
});

test("host compiles raw SFCs and updates session CSS after edits", async (t) => {
  const root = path.resolve(
    path.dirname(fileURLToPath(import.meta.url)),
    "../../integration/testss",
  );
  const facts = new TypeScriptFactsService({
    root,
    project: {
      kind: "isolated",
    },
  });
  t.onTestFinished(() => facts.close());
  const loader = new NativeDesignSystemLoader();
  loader.provide(
    0,
    "/theme.css",
    "@utility flex { display: flex; } @utility grid { display: grid; } @stillwind-slot utilities;",
  );
  const session = new NativeProjectSession(loader, true);
  const host = {
    resolve: async () => {
      expect.fail("Unexpected resolution");
    },
    read: async () => {
      expect.fail("Unexpected read");
    },
    describe: (id: string) => ({
      ...fileModuleDescriptor(root, id),
      document: {
        ...fileModuleDescriptor(root, id).document,
        uri: `untitled:${encodeURIComponent(id)}`,
      },
    }),
    watch: () => {},
    warn: () => {},
  };
  for (const [extension, source] of Object.entries(examples)) {
    const id = path.join(root, `Component.${extension}`);
    const output = await compileModule(session, () => facts, source, id, host);
    expect(output).toBeTruthy();
    expect(output?.code).toContain(`<${extension === "astro" ? "div" : "script"}`);
    expect(output!.diagnostics).toStrictEqual([]);
    expect(session.moduleCss(output!.moduleId).css).toMatch(/display:\s*flex/);
    const edited = await compileModule(
      session,
      () => facts,
      source.replace('"flex"', '"grid"').replace("'flex'", "'grid'"),
      id,
      host,
    );
    expect(edited?.code).toBeTruthy();
    expect(session.moduleCss(output!.moduleId).css).not.toMatch(/display:\s*flex/);
    expect(session.moduleCss(output!.moduleId).css).toMatch(/display:\s*grid/);
  }
});

test("SFC output fails on residual macros or mismatched attribute context", async () => {
  const compile = (input: PreparedSource) =>
    (
      compileProject(
        [input],
        cssInput(
          "@utility flex { display: flex; } @utility grid { display: grid; } @stillwind-slot utilities;",
        ),
      ) as ProjectBindingOutput
    ).modules[0]!;
  const input = await prepareSource({
    resolverId: "/src/Bad.vue",
    checkerFileName: "/src/Bad.vue",
    projectionFileName: "/src/Bad.vue",
    moduleId: "/src/Bad.vue",
    syntax: fileSourceKind("/src/Bad.vue").syntax,
    document: {
      uri: "untitled:Bad.vue",
      format: "vue",
      code: examples.vue,
    },
  });
  if (input.kind !== "projected") {
    expect.fail("expected a projected source");
  }
  const quoted = input.projection.editableRegions.find(
    (region) => region.attributeQuote === "double",
  );
  expect(quoted).toBeTruthy();
  quoted!.attributeQuote = "single";
  const wrongQuote = compile(input);
  expect(wrongQuote.code).toBe(null);
  expect(wrongQuote.diagnostics[0]?.code).toBe("SW31011");
  const leaked = await prepareSource({
    resolverId: "/src/Bad.vue",
    checkerFileName: "/src/Bad.vue",
    projectionFileName: "/src/Bad.vue",
    moduleId: "/src/Bad.vue",
    syntax: fileSourceKind("/src/Bad.vue").syntax,
    document: {
      uri: "untitled:Bad.vue",
      format: "vue",
      code: examples.vue.replace("const active", "const leaked = style; const active"),
    },
  });
  const leftover = compile(leaked);
  expect(leftover.code).toBe(null);
  expect(leftover.diagnostics[0]?.code).toBe("SW10005");
  const missingTarget = await prepareSource({
    resolverId: "/src/Bad.vue",
    checkerFileName: "/src/Bad.vue",
    projectionFileName: "/src/Bad.vue",
    moduleId: "/src/Bad.vue",
    syntax: fileSourceKind("/src/Bad.vue").syntax,
    document: {
      uri: "untitled:Bad.vue",
      format: "vue",
      code: examples.vue.replace(
        "import { style }",
        'import css from "stillwind/module-css"; import { style }',
      ),
    },
  });
  const noVirtualModule = compile(missingTarget);
  expect(noVirtualModule.code).toBe(null);
  expect(noVirtualModule.diagnostics[0]?.code).toBe("SW34002");
  expect(noVirtualModule.diagnostics[0]!.message).toMatch(/host-allocated virtual module/);
});

const extra = [
  {
    extension: "vue",
    name: "nested template expressions",
    source: `<script setup lang="ts">import { style } from "stillwind/vue"; const active = true, label = "hello", suffix="!", names=["a"]; const note=(x:string)=>x;</script><template><div v-bind="style(active ? 'flex' : 'grid', { class: names.map(name => name + suffix), '--label': JSON.stringify({ label }), '--note': note('__VLS_ctx.label') })" /></template>`,
    imports: 1,
  },
  {
    extension: "vue",
    name: "authored receiver",
    source: `<script setup lang="ts">import { style } from "stillwind/vue"; const __VLS_ctx = { active: true, style: (s: string) => ({ class: s }) };</script><template><div v-bind="style(__VLS_ctx.active ? 'flex' : 'grid')" /><div v-bind="__VLS_ctx.style('untouched')" /></template>`,
    imports: 1,
  },
  {
    extension: "vue",
    name: "template bindings shadow script bindings",
    source: `<script setup lang="ts">import { style } from "stillwind/vue"; const active = true;</script><template><div v-for="style in [() => ({})]" v-bind="style()" /><div v-for="active in [false]" v-bind="style(active ? 'flex' : 'grid')" /></template>`,
    imports: 1,
  },
  {
    extension: "svelte",
    name: "store and reactive expressions",
    source: `<script lang="ts">import { style } from "stillwind/svelte"; import {writable} from "svelte/store"; const enabled = writable(true); let label = "hello"; $: external = label + "!";</script><div {...style($enabled ? "flex" : "grid", { class: external, "--label": label })}></div>`,
    imports: 1,
  },
  {
    extension: "svelte",
    name: "rune expressions",
    source: `<script lang="ts">import { style } from "stillwind/svelte"; let active = $state(true); let external = $derived(active ? "a" : "b");</script><div {...style(active ? "flex" : "grid",{ class: external })}></div>`,
    imports: 1,
  },
  ...Object.entries(examples).map(([extension, source]) => ({
    extension,
    name: "local composition",
    source: source
      .replace("const active", 'const base = style("flex"); const active')
      .replace("style(active", "style(base, active"),
    imports: 1,
  })),
  ...Object.entries(examples).map(([extension, source]) => ({
    extension,
    name: "explicit module CSS",
    source: source.replace(
      "import { style }",
      'import css from "stillwind/module-css"; console.log(css); import { style }',
    ),
    imports: 1,
  })),
  {
    extension: "vue",
    name: "shared normal-script import",
    source: `<script>import { style } from "stillwind/vue";</script><script setup>const external='extra';</script><template><div v-bind="style('flex', { class: external })"/></template>`,
    imports: 1,
  },
  {
    extension: "svelte",
    name: "shared module-script import",
    source: `<script module>import { style } from "stillwind/svelte";</script><script>const external='extra';</script><div {...style("flex", { class: external })}></div>`,
    imports: 1,
  },
  {
    extension: "vue",
    name: "single quoted attribute",
    source: `<script setup>import { style } from "stillwind/vue"; const external = "extra";</script><template><div v-bind='style("flex", { class: external })'/></template>`,
    imports: 1,
  },
  {
    extension: "vue",
    name: "attribute whitespace",
    source: `<script setup>import { style } from "stillwind/vue"; const external="extra";</script><template><div v-bind="  style('flex', { class: external })  "/></template>`,
    imports: 1,
  },
  {
    extension: "vue",
    name: "two scripts",
    source: `<script>import { style as sm } from "stillwind/vue"; export const a = sm("flex", { class: "module" });</script><script setup>import { style } from "stillwind/vue"; const b = style("grid", { class: "instance" });</script><template><div v-bind="b"/></template>`,
    imports: 2,
  },
  {
    extension: "svelte",
    name: "two scripts",
    source: `<script module>import { style as sm } from "stillwind/svelte"; export const a=sm("flex", { class: "module" });</script><script>import { style } from "stillwind/svelte"; const b = style("grid", { class: "instance" });</script><div {...b}></div>`,
    imports: 2,
  },
  {
    extension: "astro",
    name: "client script",
    source: `<script>import { style } from "stillwind/astro"; console.log(style("flex", { class: "client" }));</script><div>🧭</div>`,
    imports: 1,
  },
  {
    extension: "astro",
    name: "server and client scripts",
    source: `---\nimport { style } from "stillwind/astro"; const a=style("flex", { class: "server" });\n---\n<div {...a}></div><script>import { style as sc } from "stillwind/astro"; console.log(sc("grid", { class: "client" }));</script>`,
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
  test(`${extension} ${name}: converter, native edits and framework compilation agree`, async () => {
    const filename = `/src/Component.${extension}`;
    const projected = await prepareSource({
      resolverId: filename,
      checkerFileName: filename,
      projectionFileName: filename,
      moduleId: filename,
      syntax: fileSourceKind(filename).syntax,
      document: {
        uri: `untitled:${filename}`,
        format: fileSourceKind(filename).format,
        code: source,
      },
    });
    projected.stylesheetImports = [
      "virtual:stillwind.css",
      "virtual:stillwind/carrier.css?revision=1",
    ];
    projected.moduleCssImport = "virtual:stillwind/module-css?test=1";
    if (projected.kind !== "projected") {
      expect.fail("expected a projected source");
    }
    expect(projected.document.code).toBe(source);
    expect(projected.projection.converter.version).toBeTruthy();
    expect(projected.projection.importRealms.length).toBeTruthy();
    const output = compileProject(
      [projected],
      cssInput(
        "@utility flex { display: flex; } @utility grid { display: grid; } @stillwind-slot utilities;",
      ),
    ) as ProjectBindingOutput;
    const module = output.modules[0]!;
    expect(module.diagnostics).toStrictEqual([]);
    if (source.includes("const base = style")) {
      expect(module.composition.length).toBe(1);
      expect(
        Buffer.from(source)
          .subarray(module.composition[0].base.start, module.composition[0].base.end)
          .toString("utf8"),
      ).toBe('style("flex")');
    }
    expect(module.code).toBeTruthy();
    const map = new TraceMap(
      moduleSourceMap(source, module.code!, filename, module.mappings, [], ""),
    );
    for (const expression of module.mappings.flatMap((mapping) => mapping.expressions)) {
      const generated = module.code!.slice(expression.generatedStart, expression.generatedEnd);
      const original = source.slice(expression.originalStart, expression.originalEnd);
      if (expression.exact) {
        expect(generated).toBe(original);
      }
      const origin = originalPositionFor(map, {
        line: module.code!.slice(0, expression.generatedStart).split("\n").length,
        column:
          expression.generatedStart -
          module.code!.lastIndexOf("\n", expression.generatedStart - 1) -
          1,
      });
      expect(origin.line).toBe(source.slice(0, expression.originalStart).split("\n").length);
      expect(origin.column).toBe(
        expression.originalStart - source.lastIndexOf("\n", expression.originalStart - 1) - 1,
      );
    }
    const explicit = source.includes("stillwind/module-css");
    expect(module.code!.split('import "virtual:stillwind.css";').length).toBe(explicit ? 1 : 2);
    expect(module.code!.split('import "virtual:stillwind/carrier.css?revision=1";').length).toBe(
      explicit ? 1 : 2,
    );
    if (explicit) {
      expect(module.code).toMatch(/import css from "virtual:stillwind\/module-css\?test=1"/);
    }
    expect(module.code!, module.code!).toContain(
      `stillwind/runtime/${extension === "vue" ? "class-object" : "class-string"}`,
    );
    expect(module.code!, module.code!).not.toContain("import { style }");
    expect(module.code!, module.code!).not.toContain("style(active");
    if (source.includes("__VLS_ctx")) {
      expect(module.code).toContain("__VLS_ctx");
    } else {
      expect(module.code!, module.code!).not.toContain("__VLS_ctx");
    }
    expect(module.code!.match(/\/runtime/g)?.length, module.code!).toBe(imports);
    if (extension === "vue") {
      const parsed = parseVue(module.code!, { filename });
      expect(parsed.errors).toStrictEqual([]);
      compileScript(parsed.descriptor, { id: "fixture", inlineTemplate: true });
    } else if (extension === "svelte") {
      compileSvelte(module.code!, { filename, generate: "client" });
    } else {
      const built = await compileAstro(module.code!, { filename });
      expect(built.diagnostics.filter((diagnostic) => diagnostic.severity === 1)).toStrictEqual([]);
    }
  });
}
