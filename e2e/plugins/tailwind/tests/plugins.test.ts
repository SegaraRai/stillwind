import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";

import { registerScenario } from "@stillwind-internal/e2e-harness/scenario";
import { stillwind } from "@stillwind/integration/vite";
import { chromium, type Browser } from "playwright";
import { build, createLogger, createServer, preview, type InlineConfig } from "vite";
import { afterAll, beforeAll, expect, test } from "vite-plus/test";

import { projectCases } from "./project-cases.ts";

registerScenario("tailwind-plugins", new URL("../", import.meta.url), (context) => {
  let browser: Browser;
  beforeAll(async () => {
    browser = await chromium.launch({ channel: "chrome", headless: true });
  });
  afterAll(async () => {
    await browser?.close();
  });

  for (const [name, expected] of projectCases) {
    test(`${name}: compiles its project in development and production`, async () => {
      const root = path.join(context.directory, "projects", name);
      const config: InlineConfig = {
        configFile: false,
        root,
        logLevel: "silent",
        customLogger: { ...createLogger("silent"), warn: (message) => context.logs.push(message) },
        plugins: [stillwind({ project: name })],
        server: { port: 0 },
        preview: { port: 0 },
        build: { minify: false, cssMinify: false },
      };
      const page = await browser.newPage();
      const errors: string[] = [];
      page.on("pageerror", (error) => errors.push(error.message));
      const assertStyles = async () => {
        await page.waitForFunction(() => document.querySelector("#target")?.getAttribute("class"));
        await writeFile(
          path.join(context.resultsDirectory, `${name}.css`),
          await page.evaluate(() =>
            [...document.styleSheets]
              .flatMap((sheet) => [...sheet.cssRules].map((rule) => rule.cssText))
              .join("\n"),
          ),
        );
        for (const [property, value] of Object.entries(expected)) {
          await expect
            .poll(() =>
              page
                .locator("#target")
                .evaluate(
                  (element, property) =>
                    getComputedStyle(element).getPropertyValue(property).trim(),
                  property,
                ),
            )
            .toBe(value);
        }
        expect(errors).toEqual([]);
      };
      const server = await createServer(config);
      try {
        await server.listen();
        await page.goto(server.resolvedUrls!.local[0]);
        await assertStyles();
        if (name === "scrollbar") {
          const file = path.join(root, "theme.css");
          const original = await readFile(file, "utf8");
          let changed = false;
          server.watcher.on("change", (changedFile) => {
            if (path.resolve(changedFile) === file) {
              changed = true;
            }
          });
          await writeFile(file, original.replace("#123456", "#654321"));
          await expect.poll(() => changed, { timeout: 10_000 }).toBe(true);
          await expect
            .poll(
              async () => {
                const css = await (
                  await fetch(
                    `${server.resolvedUrls!.local[0]}__stillwind/style.css?environment=client`,
                  )
                ).text();
                await writeFile(path.join(context.resultsDirectory, "scrollbar-hmr.css"), css);
                return css.includes("#654321");
              },
              { timeout: 10_000, message: "The CSS endpoint must use the changed plugin theme" },
            )
            .toBe(true);
          await expect
            .poll(
              () =>
                page
                  .locator("#target")
                  .evaluate((element) =>
                    getComputedStyle(element).getPropertyValue("--scrollbar-thumb").trim(),
                  ),
              { timeout: 10_000, message: context.logs.join("\n") },
            )
            .toBe("#654321");
          await writeFile(file, original);
        }
      } finally {
        await server.close();
      }
      await build(config);
      const production = await preview(config);
      try {
        await page.goto(production.resolvedUrls!.local[0]);
        await assertStyles();
      } finally {
        await page.close();
        await new Promise<void>((resolve, reject) =>
          production.httpServer.close((error) => (error ? reject(error) : resolve())),
        );
      }
    });
  }
});
