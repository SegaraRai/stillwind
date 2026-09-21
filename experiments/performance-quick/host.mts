import assert from "node:assert/strict";
import { mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { compileModule, TypeScriptFactsService } from "../../packages/host/dist/index.js";
import {
  NativeDesignSystemLoader,
  NativeProjectSession,
} from "../../packages/native/dist/index.js";

const root = fileURLToPath(new URL("../../packages/host/test/", import.meta.url));
const results = fileURLToPath(new URL("./results/", import.meta.url));
const labelIndex = process.argv.indexOf("--label");
const label = labelIndex < 0 ? "host" : process.argv[labelIndex + 1];
const loader = new NativeDesignSystemLoader({});
loader.provide(
  0,
  "root.css",
  "@utility flex {display:flex;} @utility red {color:red;} @utility blue {color:blue;} @stillwind-slot utilities;",
);
const host = {
  resolve: async () => null,
  read: async () => {
    throw new Error("Unexpected dependency read");
  },
  watch: () => {},
  warn: () => {},
};
const rows: { name: string; medianMs: number; samplesMs: number[] }[] = [];
async function measure(name: string, operation: () => Promise<unknown>, warmup = 2, count = 9) {
  for (let i = 0; i < warmup; i++) {
    await operation();
  }
  const samplesMs: number[] = [];
  for (let i = 0; i < count; i++) {
    const start = performance.now();
    await operation();
    samplesMs.push(performance.now() - start);
  }
  const medianMs = [...samplesMs].sort((a, b) => a - b)[Math.floor(count / 2)];
  rows.push({ name, medianMs, samplesMs });
  console.log(`${name}: ${medianMs.toFixed(3)} ms`);
}
const literal = "import {style} from 'stillwind/react'; export const props=style('flex red');";
const semantic =
  "import {style} from 'stillwind/react'; const token=style.animation({from:'red',to:'blue'}); const selected=token; export const props=style(selected(''));";
const fixtures: [string, string, string][] = [
  ["literal", "perf-literal.tsx", literal],
  ["semantic", "perf-semantic.tsx", semantic],
  [
    "vue",
    "perf.vue",
    `<script setup lang="ts">${literal.replace("export const", "const")}</script><template><div v-bind="props" /></template>`,
  ],
  [
    "svelte",
    "perf.svelte",
    `<script lang="ts">${literal.replace("export const", "const")}</script><div {...props}></div>`,
  ],
  [
    "astro",
    "perf.astro",
    `---\n${literal.replace("export const", "const")}\n---\n<div {...props}></div>`,
  ],
];
for (const [name, filename, originalSource] of fixtures) {
  const source = ["vue", "svelte", "astro"].includes(name)
    ? originalSource.replace("stillwind/react", `stillwind/${name}`)
    : originalSource;
  const id = path.join(root, filename);
  let facts: TypeScriptFactsService | undefined;
  const session = new NativeProjectSession(loader, true);
  const analyze = (text = source) =>
    compileModule(
      session,
      () => (facts ??= new TypeScriptFactsService(root)),
      text,
      id,
      root,
      host,
    );
  try {
    const output = await analyze();
    assert.ok(output?.code);
    assert.equal(output.diagnostics.length, 0);
    await measure(`host/${name}/unchanged`, () => analyze());
    let edit = 0;
    await measure(`host/${name}/edit`, () =>
      analyze(++edit % 2 ? source.replaceAll("red", "blue") : source),
    );
  } finally {
    facts?.close();
  }
}
await measure(
  "host/semantic/new-service-and-session",
  async () => {
    const service = new TypeScriptFactsService(root);
    try {
      return await compileModule(
        new NativeProjectSession(loader, true),
        () => service,
        semantic,
        path.join(root, "perf-semantic.tsx"),
        root,
        host,
      );
    } finally {
      service.close();
    }
  },
  1,
  5,
);
mkdirSync(results, { recursive: true });
writeFileSync(
  path.join(results, `${label}.json`),
  JSON.stringify(
    {
      timestamp: new Date().toISOString(),
      node: process.version,
      notes:
        "Minimal CSS; excludes initial dynamic import/converter load. Fresh-service case includes checker subprocess lifecycle; others reuse session and lazily created checker. Synthetic local fixtures without disk dependency graph.",
      rows,
    },
    null,
    2,
  ) + "\n",
);
