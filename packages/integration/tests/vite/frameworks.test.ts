import { mkdtempSync, writeFileSync, rmSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { stillwind } from "@stillwind/integration/vite";
import { createServer } from "vite";
import { expect, test } from "vite-plus/test";

test("raw SFC transforms retain stable producer registrations and replace linked CSS", async () => {
  const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../../..");
  const root = mkdtempSync(path.join(repositoryRoot, ".test-session-"));
  const css = path.join(root, "root.css");
  writeFileSync(
    css,
    "@utility red {color:red;} @utility blue {color:blue;} @stillwind-slot utilities;",
  );
  const plugins = stillwind({
    css,
  });
  const [plugin] = plugins;
  const server = await createServer({
    configFile: false,
    root,
    plugins: [plugins],
    logLevel: "silent",
    server: {
      middlewareMode: true,
      watch: null,
    },
  });
  try {
    const transform =
        typeof plugin.transform === "function" ? plugin.transform : plugin.transform?.handler,
      load = plugin.load,
      resolveId = plugin.resolveId;
    if (
      !(
        typeof transform === "function" &&
        typeof load === "function" &&
        typeof resolveId === "function"
      )
    ) {
      expect.fail(
        'Unexpected value: typeof transform === "function" &&\n        typeof load === "function" &&\n        typeof resolveId === "function"',
      );
    }
    const context = {
      resolve: (specifier: string, importer: string) =>
        server.pluginContainer.resolveId(specifier, importer),
      addWatchFile: () => {},
      warn: () => {},
      error: (message: string) => {
        expect.fail(message);
      },
    } as unknown as ThisParameterType<typeof transform>;
    const sources = {
      vue: `<script setup>import {style} from 'stillwind/vue';</script><template><div v-bind="style('red')"/></template>`,
      svelte: `<script>import {style} from 'stillwind/svelte';</script><div {...style('red')}></div>`,
      astro: `---\nimport {style} from 'stillwind/astro';\n---\n<div {...style('red')}></div>`,
    };
    const carriers: string[] = [];
    const typeOnly = `<script setup lang="ts">import type Css from 'stillwind/module-css';</script><template><div/></template>`;
    const untouched: unknown = await transform!.call(
      context,
      typeOnly,
      path.join(root, "Types.vue"),
    );
    if (!(untouched && typeof untouched === "object" && "code" in untouched)) {
      expect.fail(
        'Unexpected value: untouched && typeof untouched === "object" && "code" in untouched',
      );
    }
    expect(untouched.code).toBe(typeOnly);
    for (const [extension, source] of Object.entries(sources)) {
      const id = path.join(root, `Component.${extension}`);
      writeFileSync(id, source);
      expect(await transform!.call(context, source, `${id}?type=script`)).toBe(undefined);
      for (const color of ["red", "blue", "red"]) {
        const output: unknown = await transform!.call(
          context,
          source.replace("'red'", `'${color}'`),
          id,
        );
        if (!(output && typeof output === "object" && "code" in output)) {
          expect.fail('Unexpected value: output && typeof output === "object" && "code" in output');
        }
        const code = String(output.code);
        expect(code.startsWith(extension === "astro" ? "---" : "<script"), code).toBeTruthy();
        expect(code).not.toMatch(/import \{style\}/);
        const producer = code.match(/"(virtual:stillwind\/producer:[^"]+)"/)?.[1];
        expect(producer, code).toBeTruthy();
        if (color === "blue") {
          expect(producer).toBe(carriers.at(-1));
        }
        carriers.push(producer!);
        const loaded: unknown = await load!.call(context, "\0stillwind:snapshot.css");
        if (!(loaded && typeof loaded === "object" && "code" in loaded)) {
          expect.fail('Unexpected value: loaded && typeof loaded === "object" && "code" in loaded');
        }
        expect(typeof loaded.code).toBe("string");
        const className = code.match(/sw_[a-z0-9]+/)?.[0];
        expect(className).toBeTruthy();
        const rule = String(loaded.code).match(new RegExp(`\\.${className}\\{([^}]+)\\}`))?.[1];
        expect(rule, String(loaded.code)).toBeTruthy();
        expect(rule).toMatch(new RegExp(`color:\\s*${color}`));
      }
    }
  } finally {
    await server.close();
    rmSync(root, { recursive: true, force: true });
  }
});
