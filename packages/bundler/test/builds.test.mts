import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { runInNewContext } from "node:vm";

import { TraceMap, originalPositionFor, sourceContentFor } from "@jridgewell/trace-mapping";
import { rspack } from "@rspack/core";
import { stillwindRules } from "@stillwind/bundler";
import { VueLoaderPlugin } from "vue-loader";
import webpack from "webpack";

import { writeFrameworkFixture } from "../../vite/test/framework-fixtures.mts";

const require = createRequire(import.meta.url);
const packageRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
for (const backend of ["webpack", "rspack"] as const) {
  for (const framework of ["react", "preact", "solid", "vue", "svelte"] as const) {
    void test(`${backend} ${framework} preserves framework output and module CSS value ownership`, async () => {
      const root = mkdtempSync(path.join(packageRoot, ".framework-test-"));
      const sfc = framework === "vue" || framework === "svelte";
      if (sfc) writeFrameworkFixture(root, framework);
      else {
        writeFileSync(
          path.join(root, "root.css"),
          ".base{color:purple;} @utility red{color:red;background-image:url('./icon.svg');} @utility blue{color:blue;} @stillwind-slot utilities;",
        );
        writeFileSync(
          path.join(root, "icon.svg"),
          '<svg xmlns="http://www.w3.org/2000/svg"><text>asset</text></svg>',
        );
        writeFileSync(
          path.join(root, "main.ts"),
          `import css from 'stillwind/module-css'; import {style} from 'stillwind/${framework}'; import {auto} from './auto'; globalThis.result={css,auto,props:style('red',{class:'external'})};`,
        );
        writeFileSync(
          path.join(root, "auto.ts"),
          `import {style} from 'stillwind/${framework}'; export const auto=style('blue');`,
        );
      }
      const factory = backend === "webpack" ? webpack : (rspack as unknown as typeof webpack);
      const compiler = factory({
        context: root,
        mode: "production",
        target: "web",
        entry: "./main.ts",
        output: { path: path.join(root, "dist"), filename: "main.js", publicPath: "/assets/" },
        devtool: "source-map",
        optimization: { minimize: false },
        experiments: { css: true },
        resolve: {
          extensions: [".ts", ".js", ".mjs", ".vue", ".svelte"],
          conditionNames: ["svelte", "browser", "import", "default"],
        },
        plugins: framework === "vue" ? [new VueLoaderPlugin()] : [],
        module: {
          rules: [
            ...stillwindRules({ css: "./root.css" }),
            {
              test: /\.ts$/,
              use: [
                {
                  loader: require.resolve("swc-loader"),
                  options: { jsc: { parser: { syntax: "typescript" }, target: "es2022" } },
                },
              ],
            },
            { test: /\.css$/, type: "css" },
            { test: /\.svg$/, type: "asset/resource" },
            ...(framework === "vue"
              ? [{ test: /\.vue$/, loader: require.resolve("vue-loader") }]
              : []),
            ...(framework === "svelte"
              ? [
                  {
                    test: /\.svelte$/,
                    loader: require.resolve("svelte-loader"),
                    options: { compilerOptions: { dev: false } },
                  },
                ]
              : []),
          ],
        },
      });
      assert.ok(compiler);
      try {
        await new Promise<void>((resolve, reject) =>
          compiler.run((error, stats) => {
            if (error) reject(error);
            else if (!stats || stats.hasErrors())
              reject(
                new Error(
                  stats?.toString({ all: false, errors: true, errorDetails: true }) ??
                    "Missing build stats",
                ),
              );
            else resolve();
          }),
        );
        const dist = path.join(root, "dist");
        const files = readdirSync(dist);
        const css = files
          .filter((file) => file.endsWith(".css"))
          .map((file) => readFileSync(path.join(dist, file), "utf8"))
          .join("\n");
        const code = readFileSync(path.join(dist, "main.js"), "utf8");
        assert.doesNotMatch(code, /requires a Stillwind compiler integration/);
        if (sfc) {
          assert.match(css, /201,\s*22,\s*33/);
          assert.match(css, /11,\s*44,\s*205/);
          assert.doesNotMatch(css, /17,\s*155,\s*44/);
          assert.match(code, /17,\s*155,\s*44/);
        } else {
          assert.match(css, /color:\s*blue/);
          assert.doesNotMatch(css, /color:\s*red/);
          const global: { result?: { css: string; props: Record<string, string> } } = {};
          runInNewContext(code, { globalThis: global, self: {} });
          assert.ok(global.result);
          assert.match(global.result.css, /color:\s*red/);
          assert.doesNotMatch(global.result.css, /color:\s*blue/);
          assert.match(global.result.css, /\/assets\/[^)"']+\.svg/);
          assert.match(
            global.result.props[framework === "react" ? "className" : "class"]!,
            /external/,
          );
          assert.ok(files.some((file) => file.endsWith(".svg")));
        }
        assert.ok(files.some((file) => file.endsWith(".js.map")));
        const className = css.match(
          sfc
            ? /\.(sw_[0-9a-z]+)\s*\{\s*color:\s*rgb\(201,\s*22,\s*33\)/
            : /\.(sw_[0-9a-z]+)\s*\{\s*color:\s*blue/,
        )?.[1];
        assert.ok(className, "expected an automatic CSS class");
        const offset = code.indexOf(className);
        assert.ok(offset >= 0, "expected its class reference in JavaScript");
        const before = code.slice(0, offset);
        const map = new TraceMap(JSON.parse(readFileSync(path.join(dist, "main.js.map"), "utf8")));
        const position = originalPositionFor(map, {
          line: before.split("\n").length,
          column: before.length - before.lastIndexOf("\n") - 1,
        });
        assert.ok(position.source, "expected original macro source");
        const original = sourceContentFor(map, position.source);
        assert.ok(original);
        const expected = original.indexOf(sfc ? "style(active" : "style('blue')");
        assert.ok(expected >= 0, position.source);
        const originalBefore = original.slice(0, expected);
        assert.equal(position.line, originalBefore.split("\n").length);
        assert.equal(position.column, originalBefore.length - originalBefore.lastIndexOf("\n") - 1);
      } finally {
        await new Promise<void>((resolve, reject) =>
          compiler.close((error) => (error ? reject(error) : resolve())),
        );
        rmSync(root, { recursive: true, force: true });
      }
    });
  }
}
