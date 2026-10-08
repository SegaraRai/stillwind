import { mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { withStillwind } from "@stillwind/integration/next";
import type { NextConfig } from "next";
import { expect, test } from "vite-plus/test";
import webpack from "webpack";

for (const configuredRoot of [undefined, "application"]) {
  test(`Next loaders resolve ${configuredRoot ?? "the default root"} from the bundler context`, async () => {
    const root = mkdtempSync(
      path.join(path.dirname(fileURLToPath(import.meta.url)), "../.test-next-root-"),
    );
    const application = path.join(root, configuredRoot ?? "");
    mkdirSync(path.join(application, "src", "pages"), { recursive: true });
    writeFileSync(
      path.join(application, "root.css"),
      "@utility red{color:red}@utility blue{color:blue}@stillwind-slot utilities;",
    );
    writeFileSync(
      path.join(application, "src", "pages", "index.js"),
      "import {style} from 'stillwind/react';globalThis.props=style('red');",
    );
    writeFileSync(
      path.join(application, "src", "pages", "other.js"),
      "import {style} from 'stillwind/react';export const props=style('blue');",
    );
    const config = withStillwind({} as NextConfig, {
      root: configuredRoot,
      css: "root.css",
    });
    const compiler = webpack(
      config.webpack!(
        {
          context: root,
          mode: "production",
          entry: `./${configuredRoot ? configuredRoot + "/" : ""}src/pages/index.js`,
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
          module: {
            rules: [
              {
                test: /\.css$/,
                type: "css",
              },
            ],
          },
        },
        {} as Parameters<NonNullable<typeof config.webpack>>[1],
      ),
    );
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
                }),
              ),
            );
          } else {
            resolve();
          }
        }),
      );
      const css = readdirSync(path.join(root, "dist"))
        .filter((file) => file.endsWith(".css"))
        .map((file) => readFileSync(path.join(root, "dist", file), "utf8"))
        .join("");
      expect(css).toMatch(/color:\s*red/);
      expect(css).toMatch(/color:\s*blue/);
    } finally {
      await new Promise<void>((resolve, reject) =>
        compiler.close((error) => (error ? reject(error) : resolve())),
      );
      rmSync(root, { recursive: true, force: true });
    }
  });
}
