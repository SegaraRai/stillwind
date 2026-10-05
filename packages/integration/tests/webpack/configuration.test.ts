import { mkdtemp, readdir, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { runInNewContext } from "node:vm";

import { rspack } from "@rspack/core";
import { StillwindPlugin } from "@stillwind/integration/webpack";
import { expect, onTestFinished, test } from "vite-plus/test";
import webpack from "webpack";

for (const backend of ["webpack", "rspack"] as const) {
  test(`${backend} supplies explicit CSS values with automatic output disabled in the plugin`, async () => {
    const { root, build } = await compilerFixture(backend, "plugin");
    await build();
    const files = await readdir(path.join(root, "dist"));
    expect(files.filter((file) => file.endsWith(".css"))).toEqual([]);
    const global: {
      css?: string;
      automatic?: unknown;
    } = {};
    runInNewContext(await readFile(path.join(root, "dist/main.js"), "utf8"), {
      globalThis: global,
      self: {},
    });
    expect(global.automatic).toBeDefined();
    expect(global.css).toMatch(/color:\s*red/);
    expect(global.css).not.toMatch(/color:\s*blue/);
  });
}

test("the internal loader supplies explicit CSS values without a plugin", async () => {
  const { root, build } = await compilerFixture("webpack", "explicit-loader");
  await build();
  const global: { css?: string } = {};
  runInNewContext(await readFile(path.join(root, "dist/main.js"), "utf8"), {
    globalThis: global,
    self: {},
  });
  expect(global.css).toMatch(/color:\s*red/);
  expect(global.css).not.toMatch(/color:\s*blue/);
});

test("automatic output through the internal loader requires the compilation plugin", async () => {
  const { build } = await compilerFixture("webpack", "automatic-loader");
  await expect(build()).rejects.toThrow(/requires new StillwindPlugin\(options\)/);
});

async function compilerFixture(
  backend: "webpack" | "rspack",
  transport: "plugin" | "explicit-loader" | "automatic-loader",
) {
  const root = await mkdtemp(fileURLToPath(new URL("../../.test-configuration-", import.meta.url)));
  const options = {
    css: "root.css",
    automatic: transport === "automatic-loader",
    types: false as const,
  };
  const factory = backend === "webpack" ? webpack : (rspack as unknown as typeof webpack);
  const compiler = factory({
    context: root,
    mode: "production",
    target: "web",
    entry: "./main.js",
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
    plugins: transport === "plugin" ? [new StillwindPlugin(options)] : [],
    module: {
      rules:
        transport === "plugin"
          ? []
          : [
              {
                test: /\.js$/,
                use: [
                  {
                    loader: fileURLToPath(
                      import.meta.resolve("@stillwind/integration/internal/webpack-loader"),
                    ),
                    options,
                  },
                ],
              },
            ],
    },
  });
  onTestFinished(async () => {
    try {
      await new Promise<void>((resolve, reject) =>
        compiler.close((error) => (error ? reject(error) : resolve())),
      );
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });
  await Promise.all([
    writeFile(
      path.join(root, "root.css"),
      "@utility red{color:red}@utility blue{color:blue}@stillwind-slot utilities;",
    ),
    writeFile(
      path.join(root, "main.js"),
      "import css from 'stillwind/module-css';import {style} from 'stillwind/react';import './automatic.js';globalThis.css=css;globalThis.props=style('red');",
    ),
    writeFile(
      path.join(root, "automatic.js"),
      "import {style} from 'stillwind/react';globalThis.automatic=style('blue');",
    ),
  ]);
  return {
    root,
    build: () =>
      new Promise<void>((resolve, reject) =>
        compiler.run((error, stats) => {
          if (error || !stats || stats.hasErrors()) {
            reject(
              error ??
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
      ),
  };
}
