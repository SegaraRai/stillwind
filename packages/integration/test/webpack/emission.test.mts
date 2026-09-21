import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { rspack } from "@rspack/core";
import * as rspackIntegration from "@stillwind/integration/rspack";
import * as webpackIntegration from "@stillwind/integration/webpack";
import webpack from "webpack";

for (const backend of ["webpack", "rspack"] as const) {
  const { StillwindPlugin, stillwindRules } =
    backend === "webpack" ? webpackIntegration : rspackIntegration;
  void test(`${backend} links lazy and cached producers and retracts removed modules`, async () => {
    const root = mkdtempSync(
      path.join(path.dirname(fileURLToPath(import.meta.url)), "../.emission-test-"),
    );
    const put = (name: string, text: string) => writeFileSync(path.join(root, name), text);
    put(
      "root.css",
      "@utility red{color:red}@utility blue{color:blue}@utility green{color:green}@stillwind-slot utilities;",
    );
    const entry = "import {style} from 'stillwind/react';globalThis.main=style('red');";
    put("main.js", entry + "globalThis.lazy=()=>import('./lazy.js');");
    put("lazy.js", "import {style} from 'stillwind/react';export const props=style('blue');");
    put("unused.js", "import {style} from 'stillwind/react';export const props=style('green');");
    const factory = backend === "webpack" ? webpack : (rspack as unknown as typeof webpack);
    const compiler = factory({
      context: root,
      mode: "production",
      entry: "./main.js",
      cache: true,
      output: { path: path.join(root, "dist"), filename: "main.js", clean: true },
      optimization: { minimize: false },
      experiments: { css: true },
      plugins: [new StillwindPlugin()],
      module: {
        rules: [...stillwindRules({ css: "./root.css" }), { test: /\.css$/, type: "css" }],
      },
    });
    assert.ok(compiler);
    const run = () =>
      new Promise<void>((resolve, reject) =>
        compiler.run((error, stats) => {
          if (error) reject(error);
          else if (!stats || stats.hasErrors())
            reject(new Error(stats?.toString({ all: false, errors: true })));
          else resolve();
        }),
      );
    const css = () =>
      readdirSync(path.join(root, "dist"))
        .filter((file) => file.endsWith(".css"))
        .map((file) => readFileSync(path.join(root, "dist", file), "utf8"))
        .join("");
    try {
      await run();
      const cold = css();
      assert.match(cold, /color:\s*red/);
      assert.match(cold, /color:\s*blue/);
      assert.doesNotMatch(cold, /color:\s*green/);
      await run();
      assert.equal(css(), cold);
      put("main.js", entry);
      await run();
      assert.match(css(), /color:\s*red/);
      assert.doesNotMatch(css(), /color:\s*blue|color:\s*green/);
    } finally {
      await new Promise<void>((resolve, reject) =>
        compiler.close((error) => (error ? reject(error) : resolve())),
      );
      assert.ok(
        path
          .resolve(root)
          .startsWith(path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..") + path.sep),
      );
      rmSync(root, { recursive: true, force: true });
    }
  });
}
