import { mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { runInNewContext } from "node:vm";

import { TraceMap, originalPositionFor, sourceContentFor } from "@jridgewell/trace-mapping";
import { rspack } from "@rspack/core";
import * as rspackIntegration from "@stillwind/integration/rspack";
import * as webpackIntegration from "@stillwind/integration/webpack";
import { expect, test } from "vite-plus/test";
import { VueLoaderPlugin } from "vue-loader";
import webpack from "webpack";

import { writeFrameworkFixture } from "../shared/framework-fixtures.ts";

const require = createRequire(import.meta.url);
const packageRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
for (const backend of ["webpack", "rspack"] as const) {
  const { StillwindPlugin } = backend === "webpack" ? webpackIntegration : rspackIntegration;
  for (const framework of ["react", "preact", "solid", "vue", "svelte", "vanilla"] as const) {
    test(`${backend} ${framework} preserves framework output and module CSS value ownership`, async () => {
      const root = mkdtempSync(path.join(packageRoot, ".test-framework-"));
      const sfc = framework === "vue" || framework === "svelte";
      if (sfc) {
        writeFrameworkFixture(root, framework);
      } else {
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
        output: {
          path: path.join(root, "dist"),
          filename: "main.js",
          publicPath: "/assets/",
        },
        devtool: "source-map",
        optimization: {
          minimize: false,
        },
        experiments: {
          css: true,
        },
        resolve: {
          extensions: [".ts", ".js", ".mjs", ".vue", ".svelte"],
          conditionNames: ["svelte", "browser", "import", "default"],
        },
        plugins: [
          new StillwindPlugin({
            css: "./root.css",
            naming: { utilityClass: (_context, hash) => "custom-" + hash.toString("hex") },
            namingRevision: "framework-test",
          }),
          ...(framework === "vue" ? [new VueLoaderPlugin()] : []),
        ],
        module: {
          rules: [
            {
              test: /\.ts$/,
              use: [
                {
                  loader: require.resolve("swc-loader"),
                  options: {
                    jsc: {
                      parser: {
                        syntax: "typescript",
                      },
                      target: "es2022",
                    },
                  },
                },
              ],
            },
            {
              test: /\.css$/,
              type: "css",
            },
            {
              test: /\.svg$/,
              type: "asset/resource",
            },
            ...(framework === "vue"
              ? [
                  {
                    test: /\.vue$/,
                    loader: require.resolve("vue-loader"),
                  },
                ]
              : []),
            ...(framework === "svelte"
              ? [
                  {
                    test: /\.svelte$/,
                    loader: require.resolve("svelte-loader"),
                    options: {
                      compilerOptions: {
                        dev: false,
                      },
                    },
                  },
                ]
              : []),
          ],
        },
      });
      expect(compiler).toBeTruthy();
      try {
        await new Promise<void>((resolve, reject) =>
          compiler.run((error, stats) => {
            if (error) {
              reject(error);
            } else if (!stats || stats.hasErrors()) {
              reject(
                new Error(
                  stats?.toString({
                    all: false,
                    errors: true,
                    errorDetails: true,
                  }) ?? "Missing build stats",
                ),
              );
            } else {
              resolve();
            }
          }),
        );
        const dist = path.join(root, "dist");
        const files = readdirSync(dist);
        const css = files
          .filter((file) => file.endsWith(".css"))
          .map((file) => readFileSync(path.join(dist, file), "utf8"))
          .join("\n");
        const code = readFileSync(path.join(dist, "main.js"), "utf8");
        expect(code).not.toMatch(/requires a Stillwind compiler integration/);
        if (sfc) {
          expect(css).toMatch(/201,\s*22,\s*33/);
          expect(css).toMatch(/11,\s*44,\s*205/);
          expect(css).not.toMatch(/17,\s*155,\s*44/);
          expect(code).toMatch(/17,\s*155,\s*44/);
        } else {
          expect(css).toMatch(/color:\s*blue/);
          expect(css).not.toMatch(/color:\s*red/);
          const global: { result?: { css: string; props: Record<string, string> } } = {};
          runInNewContext(code, { globalThis: global, self: {} });
          expect(global.result).toBeTruthy();
          expect(global.result!.css).toMatch(/color:\s*red/);
          expect(global.result!.css).not.toMatch(/color:\s*blue/);
          expect(global.result!.css).toMatch(/\/assets\/[^)"']+\.svg/);
          expect(global.result!.props[framework === "react" ? "className" : "class"]!).toMatch(
            /external/,
          );
          expect(files.some((file) => file.endsWith(".svg"))).toBeTruthy();
        }
        expect(files.some((file) => file.endsWith(".js.map"))).toBeTruthy();
        const className = css.match(
          sfc
            ? /\.(custom-[0-9a-f]+)\s*\{\s*color:\s*rgb\(201,\s*22,\s*33\)/
            : /\.(custom-[0-9a-f]+)\s*\{\s*color:\s*blue/,
        )?.[1];
        expect(className, "expected an automatic CSS class").toBeTruthy();
        const offset = code.indexOf(className!);
        expect(offset, "expected its class reference in JavaScript").toBeGreaterThanOrEqual(0);
        const before = code.slice(0, offset);
        const map = new TraceMap(JSON.parse(readFileSync(path.join(dist, "main.js.map"), "utf8")));
        const position = originalPositionFor(map, {
          line: before.split("\n").length,
          column: before.length - before.lastIndexOf("\n") - 1,
        });
        expect(position.source, "expected original macro source").toBeTruthy();
        const original = sourceContentFor(map, position.source!);
        expect(original).toBeTruthy();
        const expected = original!.indexOf(sfc ? "style(active" : "style('blue')");
        expect(expected, position.source!).toBeGreaterThanOrEqual(0);
        const originalBefore = original!.slice(0, expected);
        expect(position.line).toBe(originalBefore.split("\n").length);
        expect(position.column).toBe(originalBefore.length - originalBefore.lastIndexOf("\n") - 1);
      } finally {
        await new Promise<void>((resolve, reject) =>
          compiler.close((error) => (error ? reject(error) : resolve())),
        );
        rmSync(root, { recursive: true, force: true });
      }
    });
  }
}
