import { mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { rspack } from "@rspack/core";
import * as rspackIntegration from "@stillwind/integration/rspack";
import * as webpackIntegration from "@stillwind/integration/webpack";
import { expect, test } from "vite-plus/test";
import webpack from "webpack";

for (const backend of ["webpack", "rspack"] as const) {
  const { StillwindPlugin } = backend === "webpack" ? webpackIntegration : rspackIntegration;

  test(`${backend} keeps generated query producers distinct and reads their transformed dependencies without executing them`, async () => {
    const root = mkdtempSync(
      path.join(path.dirname(fileURLToPath(import.meta.url)), "../.test-query-"),
    );
    const put = (name: string, code: string) => writeFileSync(path.join(root, name), code);
    put(
      "root.css",
      "@utility red{color:red}@utility blue{color:blue}@utility green{color:green}@stillwind-slot utilities;@stillwind-slot keyframes;",
    );
    put(
      "variant.js",
      "import {style} from 'stillwind/react'; export const token=style.keyframes({from:'green',to:'green'}); export const props=style('green'); throw new Error('application code executed during compilation');",
    );
    put(
      "main.js",
      "import {style} from 'stillwind/react'; import {props as first} from './variant.js?first'; import {props as second,token} from './variant.js?second'; globalThis.props=[first,second,style(token(''))];",
    );
    put(
      "variant-loader.cjs",
      "module.exports=function(source){const token=this.resourceQuery==='?first'?'red':this.resourceQuery==='?second'?'blue':'green';return source.replaceAll(\"'green'\", \"'\"+token+\"'\");};",
    );
    const factory = backend === "webpack" ? webpack : (rspack as unknown as typeof webpack);
    const compiler = factory({
      context: root,
      mode: "production",
      target: "web",
      entry: "./main.js",
      cache: true,
      output: {
        path: path.join(root, "dist"),
        filename: "main.js",
      },
      optimization: {
        minimize: false,
      },
      experiments: {
        css: true,
      },
      plugins: [
        new StillwindPlugin({
          css: "./root.css",
        }),
      ],
      module: {
        rules: [
          {
            test: /\.js$/,
            enforce: "pre",
            use: [path.join(root, "variant-loader.cjs")],
          },
          {
            test: /\.css$/,
            type: "css",
          },
        ],
      },
    });
    const run = () =>
      new Promise<void>((resolve, reject) =>
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
                }),
              ),
            );
          } else {
            resolve();
          }
        }),
      );
    try {
      for (let build = 0; build < 2; build++) {
        await run();
        const css = readdirSync(path.join(root, "dist"))
          .filter((file) => file.endsWith(".css"))
          .map((file) => readFileSync(path.join(root, "dist", file), "utf8"))
          .join("");
        expect(css).toMatch(/color:\s*red/);
        expect(css).toMatch(/color:\s*blue/);
        expect(css).not.toMatch(/color:\s*green/);
      }
    } finally {
      await new Promise<void>((resolve, reject) =>
        compiler.close((error) => (error ? reject(error) : resolve())),
      );
      rmSync(root, { recursive: true, force: true });
    }
  });

  test(`${backend} links lazy and cached producers and retracts removed modules`, async () => {
    const root = mkdtempSync(
      path.join(path.dirname(fileURLToPath(import.meta.url)), "../.test-emission-"),
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
      output: {
        path: path.join(root, "dist"),
        filename: "main.js",
        clean: true,
      },
      optimization: {
        minimize: false,
      },
      experiments: {
        css: true,
      },
      plugins: [
        new StillwindPlugin({
          css: "./root.css",
        }),
      ],
      module: {
        rules: [
          {
            test: /\.css$/,
            type: "css",
          },
        ],
      },
    });
    expect(compiler).toBeTruthy();
    const run = () =>
      new Promise<void>((resolve, reject) =>
        compiler.run((error, stats) => {
          if (error) {
            reject(error);
          } else if (!stats || stats.hasErrors()) {
            reject(new Error(stats?.toString({ all: false, errors: true })));
          } else {
            resolve();
          }
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
      expect(cold).toMatch(/color:\s*red/);
      expect(cold).toMatch(/color:\s*blue/);
      expect(cold).not.toMatch(/color:\s*green/);
      await run();
      expect(css()).toBe(cold);
      put("main.js", entry);
      await run();
      expect(css()).toMatch(/color:\s*red/);
      expect(css()).not.toMatch(/color:\s*blue|color:\s*green/);
    } finally {
      await new Promise<void>((resolve, reject) =>
        compiler.close((error) => (error ? reject(error) : resolve())),
      );
      expect(
        path
          .resolve(root)
          .startsWith(path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..") + path.sep),
      ).toBeTruthy();
      rmSync(root, { recursive: true, force: true });
    }
  });
}
