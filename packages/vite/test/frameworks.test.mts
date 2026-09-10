import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync, rmSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { createServer } from "vite-plus";

import { stillwind } from "../src/index.ts";

void test("raw SFC transforms publish immutable CSS carriers and skip framework subrequests", async () => {
  const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../..");
  const root = mkdtempSync(path.join(repositoryRoot, ".session-test-"));
  const css = path.join(root, "root.css");
  writeFileSync(
    css,
    "@utility red {color:red;} @utility blue {color:blue;} @stillwind-slot utilities;",
  );
  const plugin = stillwind({ css, types: false });
  const server = await createServer({
    configFile: false,
    root,
    plugins: [plugin],
    logLevel: "silent",
    server: { middlewareMode: true, watch: null },
  });
  try {
    const transform = plugin.transform,
      load = plugin.load,
      resolveId = plugin.resolveId;
    assert.ok(
      typeof transform === "function" &&
        typeof load === "function" &&
        typeof resolveId === "function",
    );
    const context = {
      resolve: (specifier: string, importer: string) =>
        server.pluginContainer.resolveId(specifier, importer),
      addWatchFile: () => {},
      warn: () => {},
      error: (message: string) => {
        throw new Error(message);
      },
    } as unknown as ThisParameterType<typeof transform>;
    const sources = {
      vue: `<script setup>import {style} from 'stillwind/vue';</script><template><div v-bind="style('red')"/></template>`,
      svelte: `<script>import {style} from 'stillwind/svelte';</script><div {...style('red')}></div>`,
      astro: `---\nimport {style} from 'stillwind/astro';\n---\n<div {...style('red')}></div>`,
    };
    const carriers: string[] = [];
    const typeOnly = `<script setup lang="ts">import type Css from 'stillwind/module-css';</script><template><div/></template>`;
    const untouched: unknown = await transform.call(
      context,
      typeOnly,
      path.join(root, "Types.vue"),
    );
    assert.ok(untouched && typeof untouched === "object" && "code" in untouched);
    assert.equal(untouched.code, typeOnly);
    for (const [extension, source] of Object.entries(sources)) {
      const id = path.join(root, `Component.${extension}`);
      writeFileSync(id, source);
      assert.equal(await transform.call(context, source, `${id}?type=script`), undefined);
      for (const color of ["red", "blue", "red"]) {
        const output: unknown = await transform.call(
          context,
          source.replace("'red'", `'${color}'`),
          id,
        );
        assert.ok(output && typeof output === "object" && "code" in output);
        const code = String(output.code);
        assert.ok(code.startsWith(extension === "astro" ? "---" : "<script"), code);
        assert.doesNotMatch(code, /import \{style\}/);
        const carrier = code.match(/"(virtual:stillwind\/module\.css\?ref=[^"]+)"/)?.[1];
        assert.ok(carrier, code);
        assert.ok(!carriers.includes(carrier));
        carriers.push(carrier);
        const resolved: unknown = await resolveId.call(context, carrier, id, { isEntry: false });
        assert.equal(typeof resolved, "string");
        const loaded: unknown = await load.call(context, resolved as string);
        assert.ok(loaded && typeof loaded === "object" && "code" in loaded);
        assert.ok(typeof loaded.code === "string");
        assert.match(loaded.code, new RegExp(`color:\\s*${color}`));
        assert.doesNotMatch(
          loaded.code,
          new RegExp(`color:\\s*${color === "red" ? "blue" : "red"}`),
        );
      }
    }
    // Loading an earlier identity after further transforms still returns its own payload.
    const first = await resolveId.call(context, carriers[0]!, undefined, { isEntry: false });
    const earlier = await load.call(context, first as string);
    assert.ok(earlier && typeof earlier === "object" && "code" in earlier);
    assert.match(earlier.code!.toString(), /color:\s*red/);
    assert.doesNotMatch(earlier.code!.toString(), /color:\s*blue/);
  } finally {
    await server.close();
    rmSync(root, { recursive: true, force: true });
  }
});
