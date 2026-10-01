import { existsSync, readdirSync, readFileSync } from "node:fs";
import { createRequire } from "node:module";

import { withStillwind } from "@stillwind/integration/next";
import { pluginStillwind } from "@stillwind/integration/rsbuild";
import * as rspack from "@stillwind/integration/rspack";
import { stillwindRules as turbopackRules } from "@stillwind/integration/turbopack";
import { stillwind } from "@stillwind/integration/vite";
import * as webpack from "@stillwind/integration/webpack";
import { expect, test } from "vite-plus/test";

const require = createRequire(import.meta.url);

test("distributed artifacts do not depend on Vite+", () => {
  const dist = new URL("../dist/", import.meta.url);
  for (const file of readdirSync(dist, { recursive: true, encoding: "utf8" })) {
    if (/\.(?:js|ts|map)$/.test(file)) {
      expect(readFileSync(new URL(file.replaceAll("\\", "/"), dist), "utf8"), file).not.toMatch(
        /vite-plus/,
      );
    }
  }
  const manifest = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8"));
  for (const field of ["dependencies", "peerDependencies", "optionalDependencies"]) {
    expect(manifest[field]?.["vite-plus"], field).toBe(undefined);
  }
});

test("public entrypoints and configured loaders use built ESM artifacts", async () => {
  for (const name of ["vite", "webpack", "rspack", "rsbuild", "next", "astro", "turbopack"]) {
    const entry = import.meta.resolve(`@stillwind/integration/${name}`);
    expect(entry.endsWith(`/dist/${name}/index.js`), entry).toBeTruthy();
    expect(existsSync(new URL(entry.replace(/\.js$/, ".d.ts")))).toBeTruthy();
  }
  for (const rule of webpack.stillwindRules({ css: "root.css" })) {
    for (const use of rule.use) {
      const file = typeof use === "string" ? use : use.loader;
      expect(existsSync(file), file).toBeTruthy();
    }
  }
});

test("integration subpaths expose only their tool's entrypoints", () => {
  expect(typeof stillwind).toBe("function");
  for (const backend of ["webpack", "rspack"]) {
    expect(Object.keys(require(`@stillwind/integration/${backend}`)).sort()).toStrictEqual([
      "StillwindPlugin",
      "stillwindRules",
    ]);
  }
  expect(webpack.StillwindPlugin).toBe(rspack.StillwindPlugin);
  expect(webpack.stillwindRules).toBe(rspack.stillwindRules);
  expect(() => require("@stillwind/integration")).toThrow(
    expect.objectContaining({ code: "ERR_PACKAGE_PATH_NOT_EXPORTED" }),
  );
});

test("Next configuration preserves hooks and installs shared emission", () => {
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
  expect(compiled.plugins[0]).toBe(priorPlugin);
  expect(compiled.plugins.length).toBe(1);
  expect(compiled.module.rules.length).toBe(2);
  expect(compiled.module.rules[0].use[0].loader.endsWith("loader.js")).toBeTruthy();
  expect(compiled.module.rules[1].use[0].loader.endsWith("css-loader.js")).toBeTruthy();
  const rules = config.turbopack.rules["*.{js,jsx,ts,tsx,mjs,cjs}"];
  if (!Array.isArray(rules)) {
    expect.fail("Unexpected value: Array.isArray(rules)");
  }
  expect(rules[0]).toBe(priorRule);
});

test("Rsbuild installs shared emission after framework pre-loaders", () => {
  const existing = { use: ["builtin:lightningcss-loader"] };
  const config: any = { module: { rules: [existing] } };
  pluginStillwind({ css: "root.css" }).setup({
    modifyRspackConfig(callback) {
      expect(callback(config)).toBe(config);
    },
  });
  expect(config.plugins[0]).toBeInstanceOf(rspack.StillwindPlugin);
  expect(config.module.rules[1].enforce).toBe(undefined);
  expect(existing.use.at(-1)).toBe("builtin:lightningcss-loader");
  expect(existing.use.length).toBe(2);
});

test("Turbopack rules use independent loaders and explicit application entries", () => {
  const rules = turbopackRules({ css: "root.css", entries: ["src/main.ts"] });
  expect(() => turbopackRules({ css: "root.css", entries: [] })).toThrow(/entry files/);
  for (const rule of Object.values(rules)) {
    for (const { loader, options } of rule.loaders) {
      expect(existsSync(loader)).toBeTruthy();
      expect(loader.replaceAll("\\", "/")).toMatch(/\/dist\/turbopack\//);
      expect(options.entries).toStrictEqual(["src/main.ts"]);
      expect(JSON.parse(JSON.stringify(options))).toStrictEqual(options);
    }
  }
});
