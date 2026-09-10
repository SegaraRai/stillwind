import { mkdirSync, writeFileSync } from "node:fs";

import * as astro from "@astrojs/compiler";
import * as astroRs from "@astrojs/compiler-rs";
import * as vueSfc from "@vue/compiler-sfc";
import * as vue from "@vue/language-core";
import { compile } from "svelte/compiler";
import { svelte2tsx } from "svelte2tsx";
import ts from "typescript";
const script = `import { style } from "stillwind/react";\nconst props = style('script-red');`;
const files = {
  astro: `---\n${script}\n---\n<div {...style('template-blue')}></div>`,
  svelte: `<script lang="ts">\n${script}\n</script>\n<div {...style('template-blue')}></div>`,
  vue: `<script setup lang="ts">\n${script}\n</script>\n<template><div v-bind="style('template-blue')"/></template>`,
};
mkdirSync("results/explore", { recursive: true });
async function show(name, fn) {
  try {
    const result = await fn();
    writeFileSync(`results/explore/${name}.json`, JSON.stringify(result, null, 2));
    console.log(name, result.code?.slice(0, 160), result.map && Object.keys(result.map));
  } catch (e) {
    console.log(name, "ERROR", e.message);
  }
}
for (const [name, compiler] of Object.entries({ astro, "astro-rs": astroRs })) {
  await show(name + "-tsx", () => compiler.convertToTSX(files.astro, { filename: "Case.astro" }));
  await show(name + "-runtime", () =>
    compiler.transform(files.astro, { filename: "Case.astro", sourcemap: "external" }),
  );
}
await show("svelte-ts", () =>
  svelte2tsx(files.svelte, { filename: "Case.svelte", isTsFile: true }),
);
await show("svelte-runtime", () => {
  const r = compile(files.svelte, { filename: "Case.svelte", generate: "client" });
  return r.js;
});
await show("vue-runtime", () => {
  const { descriptor } = vueSfc.parse(files.vue, { filename: "Case.vue" });
  const r = vueSfc.compileScript(descriptor, { id: "case", inlineTemplate: true });
  return { code: r.content, map: r.map };
});
await show("vue-virtual", () => {
  const options = vue.getDefaultCompilerOptions();
  const plugin = vue.createVueLanguagePlugin(ts, {}, options, (id) => id);
  const root = plugin.createVirtualCode("Case.vue", "vue", ts.ScriptSnapshot.fromString(files.vue));
  const service = plugin.typescript.getServiceScript(root);
  const code = service.code;
  return {
    code: code.snapshot.getText(0, code.snapshot.getLength()),
    extension: service.extension,
    mappings: code.mappings,
  };
});
