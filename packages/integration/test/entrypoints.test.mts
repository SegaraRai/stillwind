import assert from "node:assert/strict";
import { createRequire } from "node:module";
import test from "node:test";

import { withStillwind } from "@stillwind/integration/next";
import { pluginStillwind } from "@stillwind/integration/rsbuild";
import * as rspack from "@stillwind/integration/rspack";
import { stillwind } from "@stillwind/integration/vite";
import * as webpack from "@stillwind/integration/webpack";

const require = createRequire(import.meta.url);

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
  assert.ok(compiled.plugins[1] instanceof webpack.StillwindPlugin);
  assert.equal(compiled.module.rules.length, 4);
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
