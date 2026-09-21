import assert from "node:assert/strict";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { createRequire } from "node:module";
import test from "node:test";

import { withStillwind } from "@stillwind/integration/next";
import { pluginStillwind } from "@stillwind/integration/rsbuild";
import * as rspack from "@stillwind/integration/rspack";
import { stillwindRules as turbopackRules } from "@stillwind/integration/turbopack";
import { stillwind } from "@stillwind/integration/vite";
import * as webpack from "@stillwind/integration/webpack";

const require = createRequire(import.meta.url);

void test("distributed artifacts do not depend on Vite+", () => {
  const dist = new URL("../dist/", import.meta.url);
  for (const file of readdirSync(dist, { recursive: true, encoding: "utf8" })) {
    if (/\.(?:js|ts|map)$/.test(file)) {
      assert.doesNotMatch(
        readFileSync(new URL(file.replaceAll("\\", "/"), dist), "utf8"),
        /vite-plus/,
        file,
      );
    }
  }
  const manifest = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8"));
  for (const field of ["dependencies", "peerDependencies", "optionalDependencies"]) {
    assert.equal(manifest[field]?.["vite-plus"], undefined, field);
  }
});

void test("public entrypoints and configured loaders use built ESM artifacts", async () => {
  for (const name of ["vite", "webpack", "rspack", "rsbuild", "next", "astro", "turbopack"]) {
    const entry = import.meta.resolve(`@stillwind/integration/${name}`);
    assert.ok(entry.endsWith(`/dist/${name}/index.js`), entry);
    assert.ok(existsSync(new URL(entry.replace(/\.js$/, ".d.ts"))));
  }
  for (const rule of webpack.stillwindRules({ css: "root.css" })) {
    for (const use of rule.use) {
      const file = typeof use === "string" ? use : use.loader;
      assert.ok(existsSync(file), file);
    }
  }
});

void test("integration subpaths expose only their tool's entrypoints", () => {
  assert.equal(typeof stillwind, "function");
  for (const backend of ["webpack", "rspack"]) {
    assert.deepEqual(Object.keys(require(`@stillwind/integration/${backend}`)).sort(), [
      "StillwindPlugin",
      "stillwindRules",
    ]);
  }
  assert.equal(webpack.StillwindPlugin, rspack.StillwindPlugin);
  assert.equal(webpack.stillwindRules, rspack.stillwindRules);
  assert.throws(() => require("@stillwind/integration"), { code: "ERR_PACKAGE_PATH_NOT_EXPORTED" });
});

void test("Next configuration preserves hooks and installs shared emission", () => {
  const priorRule = { loaders: ["existing-loader"] };
  const priorPlugin = {};
  const config = withStillwind(
    {
      turbopack: { rules: { "*.{js,jsx,ts,tsx,mjs,cjs}": priorRule } },
      webpack: (value: any) => ({ ...value, plugins: [priorPlugin] }),
    },
    { css: "root.css" },
  );
  const compiled = config.webpack({});
  assert.equal(compiled.plugins[0], priorPlugin);
  assert.equal(compiled.plugins.length, 1);
  assert.equal(compiled.module.rules.length, 2);
  assert.ok(compiled.module.rules[0].use[0].loader.endsWith("loader.js"));
  assert.ok(compiled.module.rules[1].use[0].loader.endsWith("css-loader.js"));
  const rules = config.turbopack.rules["*.{js,jsx,ts,tsx,mjs,cjs}"];
  assert.ok(Array.isArray(rules));
  assert.equal(rules[0], priorRule);
});

void test("Rsbuild installs shared emission after framework pre-loaders", () => {
  const existing = { use: ["builtin:lightningcss-loader"] };
  const config: any = { module: { rules: [existing] } };
  pluginStillwind({ css: "root.css" }).setup({
    modifyRspackConfig(callback) {
      assert.equal(callback(config), config);
    },
  });
  assert.ok(config.plugins[0] instanceof rspack.StillwindPlugin);
  assert.equal(config.module.rules[1].enforce, undefined);
  assert.equal(existing.use.at(-1), "builtin:lightningcss-loader");
  assert.equal(existing.use.length, 2);
});

void test("Turbopack rules use independent loaders and explicit application entries", () => {
  const rules = turbopackRules({ css: "root.css", entries: ["src/main.ts"] });
  assert.throws(() => turbopackRules({ css: "root.css", entries: [] }), /entry files/);
  for (const rule of Object.values(rules)) {
    for (const { loader, options } of rule.loaders) {
      assert.ok(existsSync(loader));
      assert.match(loader.replaceAll("\\", "/"), /\/dist\/turbopack\//);
      assert.deepEqual(options.entries, ["src/main.ts"]);
      assert.deepEqual(JSON.parse(JSON.stringify(options)), options);
    }
  }
});
