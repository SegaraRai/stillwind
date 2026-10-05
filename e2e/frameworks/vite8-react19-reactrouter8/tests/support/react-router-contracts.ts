import fs from "node:fs/promises";
import path from "node:path";

import { expectCss, expectClassName } from "@stillwind-internal/e2e-harness/browser";
import { verifyMacroSourceMap } from "@stillwind-internal/e2e-harness/source-map";
import { type Page } from "playwright";
import { expect } from "vite-plus/test";

import type { ReactRouterApplication } from "./react-router-application.ts";

async function verify(app: ReactRouterApplication, page: Page, color: string) {
  const noJs = await app.browser.newContext({ javaScriptEnabled: false });
  try {
    const initial = await noJs.newPage();
    await initial.goto(app.url);
    await fs.writeFile(
      path.join(app.context.resultsDirectory, "router-ssr.html"),
      await initial.content(),
    );
    expect(await initial.locator("#client").evaluate((el) => getComputedStyle(el).color)).toBe(
      color,
    );
    expect(await initial.locator("#server").evaluate((el) => getComputedStyle(el).color)).toBe(
      "rgb(21, 32, 43)",
    );
  } finally {
    await noJs.close();
  }
  await page.goto(app.url);
  await page.waitForFunction(() => document.documentElement.dataset.hydrated === "yes");
  await expectCss(page, "#client", { color: color });
  await page.locator("#client").click();
  await page.waitForFunction(
    () => document.querySelector<HTMLElement>("#client")?.textContent === "1",
  );
  expect(await page.evaluate(() => window.__stillwindServerMarker)).toBe(undefined);
  const initialCss = await page.evaluate(() =>
    [...document.styleSheets]
      .flatMap((sheet) => [...sheet.cssRules].map((rule) => rule.cssText))
      .join("\n"),
  );
  await page.evaluate(() => (window.__navigationSentinel = 42));
  await page.locator("#link").click();
  await page.locator("#lazy").waitFor();
  await expectCss(page, "#lazy", { color: "rgb(90, 80, 70)" });
  const lazyClass = await page.locator("#lazy").getAttribute("class");
  expect(
    !initialCss.includes(`.${lazyClass}`),
    "Lazy CSS was present before navigation",
  ).toBeTruthy();
  expect(await page.evaluate(() => window.__navigationSentinel)).toBe(42);
  await page.locator("#back").click();
  await page.locator("#client").waitFor();
}

export async function expectRendering(app: ReactRouterApplication): Promise<void> {
  await verify(app, app.page, "rgb(11, 22, 33)");
  app.results.checks.push({
    name: "dev-ssr-hydration-navigation",
    passed: true,
  });
}

export async function expectThemeUpdate(app: ReactRouterApplication): Promise<void> {
  await app.write("root.css", app.theme("rgb(44,55,66)"));
  await expectCss(app.page, "#client", { color: "rgb(44, 55, 66)" });
  app.results.checks.push({
    name: "dev-css-edit",
    passed: true,
  });
}

export async function expectMacroRetraction(app: ReactRouterApplication): Promise<void> {
  for (let edit = 0; edit < 3; edit++) {
    await app.write("app/routes/home.jsx", app.home.replace('{...style("brand")}', ""));
    await expectClassName(app.page, "#client", "");
    expect(await app.page.locator("#client").evaluate((el) => getComputedStyle(el).color)).toBe(
      "rgb(0, 0, 0)",
    );
    await app.write("app/routes/home.jsx", app.home);
    await expectCss(app.page, "#client", { color: "rgb(11, 22, 33)" });
  }
  app.results.checks.push({
    name: "dev-macro-removal-restoration",
    passed: true,
  });
}

export async function expectServerProjectionEdit(app: ReactRouterApplication): Promise<void> {
  await app.write(
    "app/server.server.js",
    app.serverSource.replace('style("server")', 'style("lazy")'),
  );
  await expectCss(app.page, "#server", { color: "rgb(90, 80, 70)" });
  await app.write("app/server.server.js", app.serverSource);
  await expectCss(app.page, "#server", { color: "rgb(21, 32, 43)" });
  app.results.checks.push({
    name: "dev-server-css-projection-edit",
    passed: true,
  });
}

export async function expectProduction(
  app: ReactRouterApplication,
  temperature: "cold" | "warm",
): Promise<void> {
  await app.stop();
  await app.build(temperature);
  for (const entry of await fs.readdir(path.join(app.dir, "build/client"), { recursive: true })) {
    if (/\.(js|map)$/.test(entry)) {
      expect(
        !(await fs.readFile(path.join(app.dir, "build/client", entry), "utf8")).includes(
          "SERVER_CODE_ONLY",
        ),
        "Server code leaked to client artifacts",
      ).toBeTruthy();
    }
  }
  const mapped = await verifyMacroSourceMap(
    path.join(app.dir, "build/client/assets"),
    "app/routes/home.jsx",
    app.home,
    'style("brand")',
  );
  await app.serve();
  await verify(app, app.page, "rgb(44, 55, 66)");
  app.results.checks.push({
    name: `${temperature}-production-ssr-hydration-navigation`,
    passed: true,
    javascriptSourceMap: {
      line: mapped.line,
      column: mapped.column,
    },
    serverCodeExcluded: true,
    ssrWithoutJavaScript: true,
    lazyCssAbsentBeforeNavigation: true,
  });
  await app.stop();
}
