import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import path from "node:path";

import { stillwind } from "@stillwind/integration/vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { createServer } from "vite";
import { expect, onTestFinished, test } from "vite-plus/test";

test("an import that only the types condition exports does not stop the transform", async () => {
  const root = await mkdtemp(path.join(import.meta.dirname, ".test-type-only-import-"));
  onTestFinished(() => rm(root, { recursive: true, force: true }));
  const css = path.join(root, "root.css");
  await writeFile(css, "@utility red {color:red;} @stillwind-slot utilities;");
  const types = path.join(root, "node_modules/types-only");
  await mkdir(types, { recursive: true });
  await writeFile(
    path.join(types, "package.json"),
    JSON.stringify({
      name: "types-only",
      exports: {
        "./elements": {
          types: "./elements.d.ts",
        },
      },
    }),
  );
  await writeFile(path.join(types, "elements.d.ts"), "export interface Attributes {id?: string}");
  await writeFile(
    path.join(root, "Component.svelte"),
    `<script lang="ts">
import type { Attributes } from "types-only/elements";
import { style } from "stillwind/svelte";
let { id }: Attributes = $props();
</script>
<div {id} {...style("red")}></div>`,
  );
  const server = await createServer({
    configFile: false,
    root,
    logLevel: "silent",
    plugins: [stillwind({ configFile: false, css }), svelte({ configFile: false })],
    server: {
      middlewareMode: true,
      watch: null,
    },
  });
  try {
    const result = await server.transformRequest("/Component.svelte");
    expect(result?.code).toContain("sw_");
  } finally {
    await server.close();
  }
});
