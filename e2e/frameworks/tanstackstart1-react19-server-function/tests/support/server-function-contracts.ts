import fs from "node:fs/promises";
import path from "node:path";

import { expectCss, expectClassName } from "@stillwind/e2e-harness/browser";
import { verifyMacroSourceMap } from "@stillwind/e2e-harness/source-map";
import { type Page } from "playwright";
import { expect } from "vite-plus/test";

import type { ServerFunctionApplication } from "./server-function-application.ts";

async function verify(app: ServerFunctionApplication, page: Page, color: string) {
  const context = await app.browser.newContext({ javaScriptEnabled: false });
  try {
    const initial = await context.newPage();
    await initial.goto(app.url);
    expect(await initial.locator("#static").evaluate((el) => getComputedStyle(el).color)).toBe(
      color,
    );
    expect(await initial.locator("#server").evaluate((el) => getComputedStyle(el).color)).toBe(
      "rgb(20, 40, 60)",
    );
  } finally {
    await context.close();
  }
  await page.goto(app.url);
  await page.waitForFunction(() => document.documentElement.dataset.hydrated === "yes");
  expect(await page.locator("#static").evaluate((el) => getComputedStyle(el).color)).toBe(color);
  await page.locator("#client").click();
  await page.waitForFunction(
    () =>
      document.querySelector<HTMLElement>("#client")!.textContent === "1" &&
      getComputedStyle(document.querySelector<HTMLElement>("#client")!).color === "rgb(70, 80, 90)",
  );
  await page.locator("#rpc").click();
  await page.waitForFunction(
    () =>
      document.querySelector<HTMLElement>("#server")!.dataset.rpc === "SERVER_FUNCTION_SENTINEL",
  );
  const initialCss = await page.evaluate(() =>
    [...document.styleSheets]
      .flatMap((sheet) => [...sheet.cssRules].map((rule) => rule.cssText))
      .join("\n"),
  );
  await page.evaluate(() => (window.__navigationSentinel = 42));
  await page.locator("#link").click();
  await page.locator("#lazy").waitFor();
  await expectCss(page, "#lazy", { color: "rgb(90, 80, 70)" });
  expect(initialCss.includes("." + (await page.locator("#lazy").getAttribute("class")))).toBe(
    app.bundler === "rsbuild",
  );
  expect(await page.evaluate(() => window.__navigationSentinel)).toBe(42);
  await page.locator("#back").click();
  await page.locator("#static").waitFor();
}

export async function expectRendering(app: ServerFunctionApplication): Promise<void> {
  await verify(app, app.page, "rgb(11, 22, 33)");
  app.results.checks.push({
    name: "dev-ssr-hydration-server-function-lazy-navigation",
    passed: true,
  });
}

export async function expectThemeUpdate(app: ServerFunctionApplication): Promise<void> {
  await app.write("root.css", app.theme("rgb(44,55,66)"));
  await expectCss(app.page, "#static", { color: "rgb(44, 55, 66)" });
  app.results.checks.push({
    name: "dev-css-edit",
    passed: true,
  });
}

export async function expectMacroRetraction(app: ServerFunctionApplication): Promise<void> {
  await app.write("src/routes/index.tsx", app.home.replace('{...style("brand")}', ""));
  await expectClassName(app.page, "#static", "");
  await app.write(
    "src/server-style.ts",
    'import { style } from "stillwind/react";\nexport const serverProps = style("server");\n',
  );
  await app.write("src/routes/index.tsx", app.home);
  await expectCss(app.page, "#static", { color: "rgb(11, 22, 33)" });
  app.results.checks.push({
    name: "dev-macro-removal-restoration",
    passed: true,
  });
}

export async function expectServerProjectionEdit(app: ServerFunctionApplication): Promise<void> {
  await app.write(
    "src/server-style.ts",
    'import { style } from "stillwind/react";\nexport const serverProps = style("[color:rgb(30,50,70)]");\n',
  );
  await app.page.evaluate(() => (window.__navigationSentinel = 42));
  await expect
    .poll(
      async () => {
        await app.page.locator("#refresh").click();
        return app.page.locator("#server").evaluate((element) => getComputedStyle(element).color);
      },
      { timeout: 30_000 },
    )
    .toBe("rgb(30, 50, 70)");
  expect(await app.page.evaluate(() => window.__navigationSentinel)).toBe(42);
  app.results.checks.push({
    name: "dev-server-css-projection-edit",
    passed: true,
  });
}

export async function expectProduction(
  app: ServerFunctionApplication,
  temperature: "cold" | "warm",
): Promise<void> {
  await app.page.goto("about:blank");
  await app.stop();
  await app.write(
    "src/server-style.ts",
    'import { style } from "stillwind/react";\nexport const serverProps = style("server");\n',
  );
  await app.build(temperature);
  const clientDir = path.join(app.dir, app.bundler === "vite" ? ".output/public" : "dist/client");
  const cssSourceMap =
    app.bundler === "rsbuild"
      ? await verifyMacroSourceMap(
          clientDir,
          "src/routes/index.tsx",
          app.home,
          'style("brand")',
          true,
        )
      : undefined;
  const javascriptSourceMap = await verifyMacroSourceMap(
    clientDir,
    "src/routes/index.tsx",
    app.home,
    'style("brand")',
  );
  for (const file of await fs.readdir(clientDir, { recursive: true })) {
    if (/\.[cm]?js$/.test(file)) {
      expect(
        !(await fs.readFile(path.join(clientDir, file), "utf8")).includes(
          "SERVER_FUNCTION_SENTINEL",
        ),
        `server function leaked into ${file}`,
      ).toBeTruthy();
    }
  }
  await app.serve();
  await verify(app, app.page, "rgb(44, 55, 66)");
  app.results.checks.push({
    name: `${temperature}-ssr-hydration-server-function-lazy-navigation`,
    passed: true,
    javascriptSourceMap,
    cssSourceMap,
  });
  await app.page.goto("about:blank");
  await app.stop();
}

export async function expectFreshServerProjection(app: ServerFunctionApplication): Promise<void> {
  await app.write(
    "src/server-style.ts",
    'import { style } from "stillwind/react";\nexport const serverProps = style("[color:rgb(30,50,70)]");\n',
  );
  await expect
    .poll(
      async () => {
        await app.page.locator("#rpc").click();
        return app.page.locator("#server").evaluate((element) => getComputedStyle(element).color);
      },
      { timeout: 30_000 },
    )
    .toBe("rgb(30, 50, 70)");
  const context = await app.browser.newContext({ javaScriptEnabled: false });
  try {
    const page = await context.newPage();
    await page.goto(app.url);
    await expectCss(page, "#server", { color: "rgb(30, 50, 70)" });
  } finally {
    await context.close();
  }
}
