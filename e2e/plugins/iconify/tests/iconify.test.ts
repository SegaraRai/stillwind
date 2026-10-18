import path from "node:path";

import { registerScenario } from "@stillwind-internal/e2e-harness/scenario";
import { stillwind } from "@stillwind/integration/vite";
import { chromium, type Browser, type Page } from "playwright";
import { build, createLogger, createServer, preview, type InlineConfig } from "vite";
import { afterAll, beforeAll, expect, test } from "vite-plus/test";

/**
 * Measures both icons, whose parents are 200px by 120px.
 * @param page - Page of a project.
 * @returns The width and height of each icon.
 */
async function sizes(page: Page): Promise<string[][]> {
  await page.waitForFunction(() => document.querySelector("#size-first")?.getAttribute("class"));
  return Promise.all(
    ["#icon-first", "#size-first"].map((selector) =>
      page.locator(selector).evaluate((element) => {
        const style = getComputedStyle(element);
        return [style.width, style.height, style.display];
      }),
    ),
  );
}

registerScenario("iconify-plugin", new URL("../", import.meta.url), (context) => {
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

  for (const name of ["import-first", "plugin-first"]) {
    test(`${name}: built-in sizes override icon sizes in development and production`, async () => {
      const root = path.join(context.directory, "projects", name);
      const config: InlineConfig = {
        configFile: false,
        root,
        logLevel: "silent",
        customLogger: { ...createLogger("silent"), warn: (message) => context.logs.push(message) },
        plugins: [
          stillwind({
            project: name,
          }),
        ],
        server: {
          port: 0,
        },
        preview: {
          port: 0,
        },
        build: {
          minify: false,
          cssMinify: false,
        },
      };
      const expected = ["200px", "120px", "inline-block"];
      const page = await browser.newPage();
      const errors: string[] = [];
      page.on("pageerror", (error) => errors.push(error.message));
      const server = await createServer(config);
      try {
        await server.listen();
        await page.goto(server.resolvedUrls!.local[0]);
        await expect.poll(() => sizes(page)).toEqual([expected, expected]);
      } finally {
        await server.close();
      }
      await build(config);
      const production = await preview(config);
      try {
        await page.goto(production.resolvedUrls!.local[0]);
        expect(await sizes(page)).toEqual([expected, expected]);
        expect(errors).toEqual([]);
      } finally {
        await page.close();
        await new Promise<void>((resolve, reject) =>
          production.httpServer.close((error) => (error ? reject(error) : resolve())),
        );
      }
    });
  }
});
