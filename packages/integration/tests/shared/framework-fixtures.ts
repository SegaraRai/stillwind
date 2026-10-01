import { mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";

export type Framework = "vue" | "svelte" | "astro";

/** The retained fixtures can also be served for browser and HMR acceptance. */
export function writeFrameworkFixture(root: string, framework: Framework) {
  const put = (name: string, content: string) => {
    const file = path.join(root, name);
    mkdirSync(path.dirname(file), { recursive: true });
    writeFileSync(file, content);
  };
  put(
    "root.css",
    ".document-base{font-family:sans-serif;} @utility red {color:rgb(201,22,33);} @utility blue {color:rgb(11,44,205);} @utility green {color:rgb(17,155,44);} @stillwind-slot utilities;",
  );
  if (framework === "vue") {
    put("index.html", '<div id="app"></div><script type="module" src="/main.ts"></script>');
    put(
      "main.ts",
      "import {createApp} from 'vue'; import Component from './Component.vue'; createApp(Component).mount('#app');",
    );
    put(
      "Component.vue",
      `<script setup lang="ts">\nimport {ref} from 'vue';\nimport {style} from 'stillwind/vue';\nimport Explicit from './Explicit.vue';\nconst active=ref(true);\n</script>\n<template><main><h1>Vue projection</h1><button @click="active = !active">Toggle</button><div data-testid="target" v-bind="style(active ? 'red' : 'blue', {class: 'external'})">日本語 🧭</div><Explicit/></main></template>`,
    );
    put(
      "Explicit.vue",
      `<script setup lang="ts">\nimport {onMounted} from 'vue';\nimport css from 'stillwind/module-css';\nimport {style} from 'stillwind/vue';\nonMounted(() => {const sheet=document.createElement('style'); sheet.textContent=css; document.head.append(sheet);});\n</script>\n<template><div data-testid="explicit" v-bind="style('green', {class: 'explicit'})">Explicit CSS value</div></template>`,
    );
    put(
      "vite.config.mts",
      `import {defineConfig} from 'vite-plus'; import vue from '@vitejs/plugin-vue'; import {stillwind} from '@stillwind/integration/vite'; export default defineConfig({root:import.meta.dirname, plugins:[stillwind({css:import.meta.dirname+'/root.css',types:false}),vue()]});`,
    );
  } else if (framework === "svelte") {
    put("index.html", '<div id="app"></div><script type="module" src="/main.ts"></script>');
    put(
      "main.ts",
      "import {mount} from 'svelte'; import Component from './Component.svelte'; mount(Component,{target:document.querySelector('#app')!});",
    );
    put(
      "Component.svelte",
      `<script lang="ts">\nimport {style} from 'stillwind/svelte';\nimport Explicit from './Explicit.svelte';\nlet active=$state(true);\n</script>\n<main><h1>Svelte projection</h1><button onclick={() => active = !active}>Toggle</button><div data-testid="target" {...style(active ? 'red' : 'blue', {class: 'external'})}>日本語 🧭</div><Explicit/></main>`,
    );
    put(
      "Explicit.svelte",
      `<script lang="ts">\nimport css from 'stillwind/module-css';\nimport {style} from 'stillwind/svelte';\n</script>\n<svelte:head>{@html '<style>'+css+'</style>'}</svelte:head><div data-testid="explicit" {...style('green', {class: 'explicit'})}>Explicit CSS value</div>`,
    );
    put(
      "vite.config.mts",
      `import {defineConfig} from 'vite-plus'; import {svelte} from '@sveltejs/vite-plugin-svelte'; import {stillwind} from '@stillwind/integration/vite'; export default defineConfig({root:import.meta.dirname, plugins:[stillwind({css:import.meta.dirname+'/root.css',types:false}),svelte({configFile:false})]});`,
    );
  } else {
    put(
      "src/pages/index.astro",
      `---\nimport {style} from 'stillwind/astro';\nimport Explicit from '../Explicit.astro';\n---\n<html lang="en"><head><meta charset="utf-8"/><title>Astro projection</title></head><body><main><h1>Astro projection</h1><button>Toggle</button><div data-testid="target" {...style('red', {class: 'external'})}>日本語 🧭</div><Explicit/></main><script>import {style} from 'stillwind/astro'; const blue=style('blue', {class:'external'}); document.querySelector('button')?.addEventListener('click',()=>{const target=document.querySelector('[data-testid=target]'); if(target) target.className=blue.class;});</script></body></html>`,
    );
    put(
      "src/Explicit.astro",
      `---\nimport css from 'stillwind/module-css';\nimport {style} from 'stillwind/astro';\n---\n<style is:inline set:html={css}></style><div data-testid="explicit" {...style('green', {class: 'explicit'})}>Explicit CSS value</div>`,
    );
    put(
      "astro.config.mjs",
      `import {defineConfig} from 'astro/config'; import {stillwind} from '@stillwind/integration/vite'; export default defineConfig({root:import.meta.dirname, vite:{plugins:[stillwind({css:import.meta.dirname+'/root.css',types:false})]},build:{inlineStylesheets:'never'}});`,
    );
  }
}
