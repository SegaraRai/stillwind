import path from "node:path";
import { setTimeout as delay } from "node:timers/promises";

import { expectCss, expectClassName } from "@stillwind-internal/e2e-harness/browser";
import { verifyMacroSourceMap } from "@stillwind-internal/e2e-harness/source-map";
import { type Page } from "playwright";
import { expect } from "vite-plus/test";

import type { SolidstartApplication } from "./solidstart-application.ts";

async function verify(app: SolidstartApplication, page: Page, color: string) {
  const ssr = await app.browser.newContext({ javaScriptEnabled: false });
  try {
    const initial = await ssr.newPage();
    await initial.goto(app.url);
    expect(await initial.locator("#static").evaluate((el) => getComputedStyle(el).color)).toBe(
      color,
    );
    expect(await initial.locator("#client").textContent()).toBe("-1");
  } finally {
    await ssr.close();
  }
  await page.goto(app.url);
  await page.waitForFunction(() => document.documentElement.dataset.hydrated === "yes");
  expect(await page.locator("#static").evaluate((el) => getComputedStyle(el).color)).toBe(color);
  expect(await page.locator("#client").evaluate((el) => getComputedStyle(el).color)).toBe(color);
  await page.locator("#client").click();
  await page.waitForFunction(
    () =>
      document.querySelector<HTMLElement>("#client")!.textContent === "1" &&
      getComputedStyle(document.querySelector<HTMLElement>("#client")!).color === "rgb(70, 80, 90)",
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
  expect(
    !initialCss.includes("." + (await page.locator("#lazy").getAttribute("class"))),
  ).toBeTruthy();
  expect(await page.evaluate(() => window.__navigationSentinel)).toBe(42);
  await page.locator("#back").click();
  await page.locator("#static").waitFor();
}

export async function expectRendering(app: SolidstartApplication): Promise<void> {
  await verify(app, app.page, "rgb(11, 22, 33)");
  app.results.checks.push({
    name: "dev-ssr-hydration-reactivity-lazy-navigation",
    passed: true,
  });
}

export async function expectThemeUpdate(app: SolidstartApplication): Promise<void> {
  await app.write("root.css", app.theme("rgb(44,55,66)"));
  await expectCss(app.page, "#static", { color: "rgb(44, 55, 66)" });
  app.results.checks.push({
    name: "dev-css-edit",
    passed: true,
  });
}

export async function expectMacroRetraction(app: SolidstartApplication): Promise<void> {
  await app.write("src/routes/index.jsx", app.home.replace('{...style("brand")}', ""));
  await expectClassName(app.page, "#static", "");
  await delay(300);
  await app.write("src/routes/index.jsx", app.home);
  await expectCss(app.page, "#static", { color: "rgb(11, 22, 33)" });
  app.results.checks.push({
    name: "dev-macro-removal-restoration",
    passed: true,
  });
}

export async function expectProduction(
  app: SolidstartApplication,
  temperature: "cold" | "warm",
): Promise<void> {
  await app.stop();
  await app.build(temperature);
  const javascriptSourceMap = await verifyMacroSourceMap(
    path.join(app.dir, ".output/public"),
    "src/routes/index.jsx",
    app.home,
    'style("brand")',
  );
  await app.serve();
  await verify(app, app.page, "rgb(44, 55, 66)");
  app.results.checks.push({
    name: `${temperature}-ssr-hydration-reactivity-lazy-navigation`,
    passed: true,
    javascriptSourceMap,
  });
  await app.stop();
}
