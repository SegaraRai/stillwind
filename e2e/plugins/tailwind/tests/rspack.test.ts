import { readdir, readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";

import type { Stats } from "@rspack/core";
import { registerScenario } from "@stillwind-internal/e2e-harness/scenario";
import { chromium, type Browser } from "playwright";
import { afterAll, beforeAll, expect, test } from "vite-plus/test";

import { projectCases } from "./project-cases.ts";

// Rspack requires the loader, which must share the plugin's module instance.
const require = createRequire(import.meta.url);
const { rspack } = require("@rspack/core") as typeof import("@rspack/core");
const { StillwindPlugin } =
  require("@stillwind/integration/rspack") as typeof import("@stillwind/integration/rspack");

/**
 * Builds a project for production with Rspack.
 *
 * @param root - Project directory.
 * @param project - Name of the project in `stillwind.config.ts`.
 * @returns The output directory.
 */
async function buildProject(root: string, project: string): Promise<string> {
  const output = path.join(root, "rspack-dist");
  const compiler = rspack({
    context: root,
    plugins: [
      new StillwindPlugin({
        project,
      }),
    ],
    mode: "production",
    optimization: {
      minimize: false,
    },
    entry: "./main.ts",
    output: {
      path: output,
      filename: "main.js",
      cssFilename: "[name].css",
    },
    resolve: {
      extensions: [".ts", ".js"],
    },
    module: {
      rules: [
        {
          test: /\.css$/,
          type: "css",
        },
        {
          test: /\.ts$/,
          loader: "builtin:swc-loader",
          options: {
            jsc: {
              parser: {
                syntax: "typescript",
              },
            },
          },
        },
      ],
    },
  });
  const stats = await new Promise<Stats | undefined>((resolve, reject) =>
    compiler.run((error, result) => (error ? reject(error) : resolve(result))),
  );
  await new Promise<void>((resolve, reject) =>
    compiler.close((error) => (error ? reject(error) : resolve())),
  );
  expect(stats?.hasErrors(), stats?.toString()).toBe(false);
  return output;
}

registerScenario("tailwind-plugins-rspack", new URL("../", import.meta.url), (context) => {
  let browser: Browser;
  beforeAll(async () => {
    browser = await chromium.launch({
      channel: "chrome",
      headless: true,
    });
  });
  afterAll(async () => {
    await browser?.close();
  });

  for (const [name, expected] of projectCases) {
    test(`${name}: compiles its project with Rspack`, async () => {
      const root = path.join(context.directory, "projects", name);
      const output = await buildProject(root, name);
      const styles = await Promise.all(
        (await readdir(output))
          .filter((file) => file.endsWith(".css"))
          .map((file) => readFile(path.join(output, file), "utf8")),
      );
      const page = await browser.newPage();
      try {
        const body = /<body>([\s\S]*)<script/.exec(await readFile(`${root}/index.html`, "utf8"))!;
        await page.setContent(`<style>${styles.join("\n")}</style>${body[1]}`);
        await page.addScriptTag({
          path: path.join(output, "main.js"),
        });
        await page.waitForFunction(() => document.querySelector("#target")?.getAttribute("class"));
        for (const [property, value] of Object.entries(expected)) {
          await expect
            .poll(
              () =>
                page
                  .locator("#target")
                  .evaluate(
                    (element, property) =>
                      getComputedStyle(element).getPropertyValue(property).trim(),
                    property,
                  ),
              {
                message: property,
              },
            )
            .toBe(value);
        }
      } finally {
        await page.close();
      }
    });
  }
});
