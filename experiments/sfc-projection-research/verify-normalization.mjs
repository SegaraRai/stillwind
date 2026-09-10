import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { writeFileSync } from "node:fs";

import { adapters } from "./adapters.mjs";
import { fixtures } from "./fixtures.mjs";

const inputs = [];
const dynamicFixtures = [
  {
    language: "vue",
    name: "dynamic-expressions",
    source: `<script setup lang="ts">
import { style as sx } from 'stillwind/vue';
import { ref } from 'vue';
const active = ref(true), label = 'hello', suffix = '!';
const names = ['a'];
</script>
<template><div v-bind="sx(active ? 'flex' : 'grid', {class: names.map(name => name + suffix), 'aria-label': JSON.stringify({label}), 'data-note': '__VLS_ctx.label'})" /></template>`,
  },
  {
    language: "vue",
    name: "user-receiver",
    source: `<script setup lang="ts">
import { style } from 'stillwind/vue';
const __VLS_ctx = { active: true, style: (s: string) => ({class: s}) };
</script>
<template><div v-bind="style(__VLS_ctx.active ? 'flex' : 'grid')" /><div v-bind="__VLS_ctx.style('untouched')" /></template>`,
  },
  {
    language: "vue",
    name: "template-shadow",
    source: `<script setup lang="ts">
import { style } from 'stillwind/vue';
const active = true;
</script>
<template><div v-for="style in [() => ({})]" v-bind="style()" /><div v-for="active in [false]" v-bind="style(active ? 'flex' : 'grid')" /></template>`,
  },
  {
    language: "svelte",
    name: "reactive-expressions",
    source: `<script lang="ts">
import { style } from 'stillwind/svelte';
import { writable } from 'svelte/store';
const enabled = writable(true);
let label = 'hello';
$: external = label + '!';
</script>
<div {...style($enabled ? 'flex' : 'grid', {class: external, 'aria-label': label})}></div>`,
  },
  {
    language: "svelte",
    name: "rune-expressions",
    source: `<script lang="ts">
import { style } from 'stillwind/svelte';
let active = $state(true);
let external = $derived(active ? 'a' : 'b');
</script>
<div {...style(active ? 'flex' : 'grid', {class: external})}></div>`,
  },
];
for (const fixture of [...fixtures, ...dynamicFixtures]) {
  const name = { astro: "astro-tsx", svelte: "svelte2tsx", vue: "vue-language-core" }[
    fixture.language
  ];
  const adapter = adapters.find((adapter) => adapter.name === name);
  const original = fixture.source.replaceAll("stillwind/react", `stillwind/${fixture.language}`);
  const result = await adapter.convert(original, `${fixture.name}.${fixture.language}`);
  const rawMap = result.map
    ? JSON.parse(typeof result.map === "string" ? result.map : JSON.stringify(result.map))
    : null;
  inputs.push({
    name: `${fixture.language}/${fixture.name}`,
    original,
    generated: result.code,
    framework: { astro: "Astro", svelte: "Svelte", vue: "Vue" }[fixture.language],
    map: rawMap
      ? { kind: "v3", json: JSON.stringify(rawMap), source: rawMap.sources[0] }
      : { kind: "volar", mappings: result.mappings },
  });
}
const results = JSON.parse(
  execFileSync(
    "cargo",
    ["run", "--quiet", "-p", "stillwind-js-oxc", "--example", "projection_probe"],
    {
      input: JSON.stringify(inputs),
      encoding: "utf8",
      maxBuffer: 32 * 1024 * 1024,
      env: { ...process.env, CARGO_INCREMENTAL: "0" },
    },
  ),
);
assert(
  results.every((result) => !result.error),
  JSON.stringify(results.filter((result) => result.error)),
);
const slice = (source, span) => Buffer.from(source).subarray(span.start, span.end).toString();
const summary = results.map((result, index) => {
  const fixture = [...fixtures, ...dynamicFixtures][index];
  assert.equal(
    result.ir.style_calls.length,
    fixture.probes?.filter((probe) => probe.kind === "call").length ?? 1,
    result.name,
  );
  assert.equal(
    result.diagnostics.length,
    result.name.endsWith("/scoped-expression") ? 1 : 0,
    result.name,
  );
  return {
    name: result.name,
    diagnostics: result.diagnostics.map(({ code }) => code),
    calls: result.ir.style_calls.map((call) => ({
      text: slice(inputs[index].original, call.span),
      reused: call.inputs.flatMap((input) => {
        if (input.ConditionalUtilityString)
          return [slice(inputs[index].original, input.ConditionalUtilityString.guard)];
        if (input.ExternalClass) return [slice(inputs[index].original, input.ExternalClass)];
        if (input.Attributes)
          return input.Attributes.map(({ value }) => slice(inputs[index].original, value));
        return [];
      }),
    })),
  };
});
const reused = (name) => summary.find((row) => row.name === name).calls[0].reused;
assert.deepEqual(reused("vue/dynamic-expressions"), [
  "active",
  "active",
  "names.map(name => name + suffix)",
  "JSON.stringify({label})",
  "'__VLS_ctx.label'",
]);
assert.deepEqual(reused("vue/user-receiver"), ["__VLS_ctx.active", "__VLS_ctx.active"]);
assert.deepEqual(reused("vue/template-shadow"), ["active", "active"]);
assert.deepEqual(reused("svelte/reactive-expressions"), [
  "$enabled",
  "$enabled",
  "external",
  "label",
]);
assert.deepEqual(reused("svelte/rune-expressions"), ["active", "active", "external"]);
writeFileSync("results/normalization-verification.json", JSON.stringify(summary, null, 2) + "\n");
console.log(
  `Validated ${results.length} real converter projections, including nested expressions, original receivers, template shadowing, stores and runes.`,
);
