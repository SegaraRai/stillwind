import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { rspack } from "@rspack/core";
import { stillwind, type StillwindOptions } from "@stillwind/integration/vite";
import { StillwindPlugin, stillwindRules } from "@stillwind/integration/webpack";
import { build } from "vite";
import webpack from "webpack";

for (const naming of [undefined, { utilityClass: () => "shared-name" }]) {
  void test(`adapters share compiler options, identities and vocabulary (${naming ? "custom" : "default"} naming)`, async () => {
    const root = mkdtempSync(path.join(path.dirname(fileURLToPath(import.meta.url)), ".parity-"));
    writeFileSync(path.join(root, "index.html"), '<script type="module" src="/main.js"></script>');
    writeFileSync(
      path.join(root, "main.js"),
      'import {style} from "stillwind/react";console.log(style("red"));',
    );
    writeFileSync(path.join(root, "root.css"), "@utility red{color:red}@stillwind-slot utilities;");
    const options: StillwindOptions = {
      theme: "root.css",
      classNamespace: "parity",
      prefix: "--test-",
      hashLength: 12,
      naming,
      namingRevision: naming ? "custom-v1" : "",
      types: "vocabulary.d.ts",
    };
    try {
      const result = await build({
        root,
        configFile: false,
        logLevel: "silent",
        plugins: [stillwind(options)],
        build: { write: false, minify: false, cssMinify: false },
      });
      assert.ok("output" in result);
      const css = result.output
        .flatMap((item) =>
          item.type === "asset" && item.fileName.endsWith(".css") ? [String(item.source)] : [],
        )
        .join("");
      const selector = css.match(/\.([\w-]+)\s*\{color:\s*red/)?.[1];
      assert.ok(selector, css);
      if (naming) {
        assert.equal(selector, "shared-name");
      }
      const vocabulary = readFileSync(path.join(root, "vocabulary.d.ts"), "utf8");
      for (const backend of [webpack, rspack as unknown as typeof webpack]) {
        const compiler = backend({
          context: root,
          mode: "production",
          entry: "./main.js",
          output: { path: path.join(root, "dist"), filename: "main.js", clean: true },
          optimization: { minimize: false },
          experiments: { css: true },
          plugins: [new StillwindPlugin()],
          module: { rules: [...stillwindRules(options), { test: /\.css$/, type: "css" }] },
        });
        assert.ok(compiler);
        try {
          await new Promise<void>((resolve, reject) =>
            compiler.run((error, stats) => {
              if (error || !stats || stats.hasErrors()) {
                reject(error ?? new Error(stats?.toString({ all: false, errors: true })));
              } else {
                resolve();
              }
            }),
          );
          const outputCss = readdirSync(path.join(root, "dist"))
            .filter((file) => file.endsWith(".css"))
            .map((file) => readFileSync(path.join(root, "dist", file), "utf8"))
            .join("");
          assert.ok(outputCss.includes(`.${selector}`), outputCss);
          assert.ok(readFileSync(path.join(root, "dist/main.js"), "utf8").includes(selector));
          assert.equal(readFileSync(path.join(root, "vocabulary.d.ts"), "utf8"), vocabulary);
        } finally {
          await new Promise<void>((resolve, reject) =>
            compiler.close((error) => (error ? reject(error) : resolve())),
          );
        }
      }
    } finally {
      assert.ok(root.startsWith(path.dirname(fileURLToPath(import.meta.url)) + path.sep));
      rmSync(root, { recursive: true, force: true });
    }
  });
}
