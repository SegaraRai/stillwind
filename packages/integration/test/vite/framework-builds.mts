import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { stillwind as astroIntegration } from "@stillwind/integration/astro";
import { stillwind } from "@stillwind/integration/vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import vue from "@vitejs/plugin-vue";
import { build as astroBuild } from "astro";
import { build, type PluginOption } from "vite";

import { writeFrameworkFixture, type Framework } from "../shared/framework-fixtures.mts";

const packageRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
for (const framework of ["vue", "svelte", "astro"] satisfies Framework[]) {
  void test(`${framework} official production build consumes native SFC edits and explicit CSS values`, async () => {
    const root = mkdtempSync(path.join(packageRoot, ".framework-test-"));
    writeFrameworkFixture(root, framework);
    const entry = path.join(
      root,
      framework === "astro" ? "src/pages/index.astro" : `Component.${framework}`,
    );
    const directory = path.dirname(entry);
    const declarations = `import {style} from 'stillwind/${framework}';export const red=style('red'),blue=style('blue');`;
    writeFileSync(
      path.join(directory, `StyleBase.${framework}`),
      framework === "astro"
        ? `---\n${declarations}\n---`
        : `<script ${framework === "svelte" ? "module " : ""}lang="ts">${declarations}${framework === "vue" ? "export default {};" : ""}</script>`,
    );
    writeFileSync(
      path.join(directory, "style-values.ts"),
      `export {red,blue} from './StyleBase.${framework}';export function pass<T>(value:T):T{return value;}`,
    );
    writeFileSync(
      entry,
      readFileSync(entry, "utf8")
        .replace(
          "import Explicit",
          "import {red,blue,pass} from './style-values.ts';\nimport Explicit",
        )
        .replace("style(active ? 'red' : 'blue',", "style(pass(active ? red : blue),")
        .replace("style('red',", "style(pass(red),"),
    );
    try {
      const plugin = stillwind({ css: path.join(root, "root.css"), types: false });
      if (framework === "astro") {
        const config: Parameters<typeof astroBuild>[0] = {
          root,
          configFile: false,
          logLevel: "silent",
          integrations: [astroIntegration({ css: path.join(root, "root.css"), types: false })],
          build: { inlineStylesheets: "never" },
        };
        await astroBuild(config);
      } else {
        const config: Parameters<typeof build>[0] = {
          root,
          configFile: false,
          logLevel: "silent",
          plugins: [
            plugin,
            (framework === "vue"
              ? vue()
              : svelte({ configFile: false })) as unknown as PluginOption,
          ],
          build: { minify: false, cssMinify: false, sourcemap: true },
        };
        await build(config);
      }
      const dist = path.join(root, "dist");
      const files = readdirSync(dist, { recursive: true }).map(String);
      const css = files
        .filter((file) => file.endsWith(".css"))
        .map((file) => readFileSync(path.join(dist, file), "utf8"))
        .join("\n");
      assert.match(css, /(?:201,\s*22,\s*33|#c91621)/i, css);
      assert.match(css, /(?:11,\s*44,\s*205|#0b2ccd)/i, css);
      assert.doesNotMatch(css, /(?:17,\s*155,\s*44|#119b2c)/i);
      const html = readFileSync(path.join(dist, "index.html"), "utf8");
      const js = files
        .filter((file) => file.endsWith(".js"))
        .map((file) => readFileSync(path.join(dist, file), "utf8"))
        .join("\n");
      assert.doesNotMatch(js, /stillwind\/(?:vue|svelte|astro)["']/);
      assert.doesNotMatch(js, /requires a Stillwind compiler integration/);
      assert.match(framework === "astro" ? html : js, /(?:17,\s*155,\s*44|#119b2c)/i);
      if (process.env.STILLWIND_KEEP_FRAMEWORK_FIXTURES) {
        console.log(`${framework} fixture: ${root}`);
      }
    } finally {
      if (!process.env.STILLWIND_KEEP_FRAMEWORK_FIXTURES) {
        rmSync(root, { recursive: true, force: true });
      }
    }
  });
}
