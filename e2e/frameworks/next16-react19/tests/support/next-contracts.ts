import fs from "node:fs/promises";
import path from "node:path";

import {
  expectCss,
  expectClassName,
  waitForHydration,
} from "@stillwind-internal/e2e-harness/browser";
import { verifyMacroSourceMap } from "@stillwind-internal/e2e-harness/source-map";
import { type Page } from "playwright";
import { expect } from "vite-plus/test";

import type { NextApplication } from "./next-application.ts";

async function verifyPage(app: NextApplication, page: Page, color: string) {
  const prefetch = page.waitForResponse(
    (response) =>
      new URL(response.url()).pathname === "/lazy" &&
      response.request().headers()["next-router-prefetch"] === "1",
  );
  await page.goto(app.url);
  await expectCss(page, "#server", { color });
  expect(await page.locator("#client").evaluate((el) => getComputedStyle(el).color)).toBe(
    "rgb(70, 80, 90)",
  );
  await waitForHydration(page);
  await page.locator("#client").click();
  await page.waitForFunction(
    () => document.querySelector<HTMLElement>("#client")?.textContent === "1",
  );
  expect(await page.locator("#composed").evaluate((el) => getComputedStyle(el).marginTop)).toBe(
    "20px",
  );
  expect(await page.locator("#composed").evaluate((el) => getComputedStyle(el).paddingTop)).toBe(
    "7px",
  );
  expect(await page.locator("#explicit").evaluate((el) => getComputedStyle(el).paddingTop)).toBe(
    "19px",
  );
  expect(
    await page.locator("#explicit").evaluate((el) => getComputedStyle(el).backgroundImage),
  ).toMatch(/\/_next\/static\/media\//);
  const flight = await prefetch;
  expect(flight.status()).toBe(200);
  expect(flight.headers()["content-type"]).toMatch(/text\/x-component/);
  expect(await flight.text()).not.toMatch(/STILLWIND_SERVER_IMPLEMENTATION_SENTINEL/);
  await page.locator("#link").click();
  await page.locator("#lazy").waitFor();
  expect(await page.locator("#lazy").evaluate((el) => getComputedStyle(el).color)).toBe(
    "rgb(90, 80, 70)",
  );
  await page.locator("#back").click();
  await page.locator("#server").waitFor();
  return { color, lazy: "rgb(90, 80, 70)", prefetched: true, flight: true };
}

export async function expectRscHydration(app: NextApplication): Promise<void> {
  await app.page.goto(app.url);
  await expectCss(app.page, "#server", { color: "rgb(11, 22, 33)" });
  await waitForHydration(app.page);
  await app.page.locator("#client").click();
  await app.page.waitForFunction(
    () => document.querySelector<HTMLElement>("#client")?.textContent === "1",
  );
  app.results.checks.push({
    name: `next-${app.mode}-rsc-client-hydration`,
    passed: true,
  });
}

export async function expectThemeUpdate(app: NextApplication): Promise<void> {
  await waitForHydration(app.page);
  await app.page.locator("#client").click();
  await expect.poll(() => app.page.locator("#client").textContent()).toBe("1");
  await app.write("root.css", app.theme("rgb(44,55,66)"));
  await expectCss(app.page, "#server", { color: "rgb(44, 55, 66)" });
  expect(await app.page.locator("#client").textContent()).toBe("1");
  app.results.checks.push({
    name: `next-${app.mode}-css-edit`,
    passed: true,
  });
}

export async function expectMacroRetraction(app: NextApplication): Promise<void> {
  const pageSource = await fs.readFile(path.join(app.dir, "app/page.jsx"), "utf8");
  await app.write(
    "app/page.jsx",
    pageSource
      .replace('import { style } from "stillwind/react";', "")
      .replace('{...style("text-brand")}', ""),
  );
  await expectClassName(app.page, "#server", "");
  expect(await app.page.locator("#server").evaluate((el) => getComputedStyle(el).color)).toBe(
    "rgb(0, 0, 0)",
  );
  await app.write("app/page.jsx", pageSource);
  await expectCss(app.page, "#server", { color: "rgb(11, 22, 33)" });
  app.results.checks.push({
    name: `next-${app.mode}-macro-removal-restoration`,
    passed: true,
  });
}

export async function expectLazyNavigation(app: NextApplication): Promise<void> {
  await app.page.locator("#link").click();
  await app.page.locator("#lazy").waitFor();
  const color = await app.page.locator("#lazy").evaluate((el) => getComputedStyle(el).color);
  expect(color).toBe("rgb(90, 80, 70)");
  app.results.checks.push({
    name: `next-${app.mode}-client-navigation`,
    passed: true,
    color,
  });
}

export async function expectCompositionEdit(app: NextApplication): Promise<void> {
  await app.page.goto(app.url);
  await expectCss(app.page, "#composed", { marginTop: "20px" });
  await app.write(
    "shared/derived.ts",
    "import { style } from \"stillwind/react\";import {base} from './base';export const derived=style(base,'m-8');",
  );
  await expectCss(app.page, "#composed", { marginTop: "32px" });
  await app.write(
    "shared/derived.ts",
    "import { style } from \"stillwind/react\";import {base} from './base';export const derived=style(base,'override');",
  );
  await expectCss(app.page, "#composed", { marginTop: "20px" });
  app.results.checks.push({
    name: `next-${app.mode}-cross-module-composition-edit`,
    passed: true,
  });
}

export async function expectRouteRetraction(app: NextApplication): Promise<void> {
  const cssText = () =>
    app.page.evaluate(() =>
      [...document.styleSheets]
        .flatMap((sheet) => [...sheet.cssRules].map((rule) => rule.cssText))
        .join("\n"),
    );
  await app.write(
    "app/added/page.jsx",
    'import { style } from "stillwind/react";export default function Added(){return <div {...style("m-[137px]")}>added</div>}',
  );
  for (let attempt = 0; attempt < 100 && !(await cssText()).includes("137px"); attempt++) {
    await app.delay(200);
  }
  expect(await cssText()).toMatch(/137px/);
  await fs.unlink(path.join(app.dir, "app/added/page.jsx"));
  for (let attempt = 0; attempt < 100 && (await cssText()).includes("137px"); attempt++) {
    await app.delay(200);
  }
  expect(await cssText()).not.toMatch(/137px/);
  app.results.checks.push({
    name: "next-turbopack-route-addition-deletion",
    passed: true,
  });
}

export async function expectProduction(
  app: NextApplication,
  temperature: "cold" | "warm",
): Promise<void> {
  await app.stop();
  await app.build(temperature);
  let serverContainsSentinel = false;
  for (const file of await fs.readdir(path.join(app.dir, ".next/server"), { recursive: true })) {
    if (
      file.endsWith(".js") &&
      (await fs.readFile(path.join(app.dir, ".next/server", file), "utf8")).includes(
        "STILLWIND_SERVER_IMPLEMENTATION_SENTINEL",
      )
    ) {
      serverContainsSentinel = true;
    }
  }
  expect(serverContainsSentinel, "The server implementation must remain in the server build").toBe(
    true,
  );
  for (const file of await fs.readdir(path.join(app.dir, ".next/static"), { recursive: true })) {
    if (/\.(?:js|map|css)$/.test(file)) {
      expect(await fs.readFile(path.join(app.dir, ".next/static", file), "utf8"), file).not.toMatch(
        /STILLWIND_SERVER_IMPLEMENTATION_SENTINEL/,
      );
    }
  }
  app.results.checks.push({
    name: `next-${app.mode}-${temperature}-server-only-isolation`,
    passed: true,
  });
  const cssSourceMap = await verifyMacroSourceMap(
    path.join(app.dir, ".next/static"),
    "app/client.jsx",
    await fs.readFile(path.join(app.dir, "app/client.jsx"), "utf8"),
    'style("client")',
    true,
  );
  const javascriptSourceMap = await verifyMacroSourceMap(
    path.join(app.dir, ".next/static"),
    "app/client.jsx",
    await fs.readFile(path.join(app.dir, "app/client.jsx"), "utf8"),
    'style("client")',
    false,
    undefined,
    app.mode === "turbopack" ? "call-span" : "start",
  );
  await app.serve();
  const ssr = await app.browser.newPage({ javaScriptEnabled: false });
  try {
    await ssr.goto(app.url);
    for (const [selector, property, expected] of [
      ["#server", "color", "rgb(44, 55, 66)"],
      ["#client", "color", "rgb(70, 80, 90)"],
      ['[data-testid="main-composed"]', "marginTop", "12px"],
      ['[data-testid="main-conditional"]', "marginTop", "40px"],
    ]) {
      expect(
        await ssr
          .locator(selector)
          .evaluate(
            (element, property) => getComputedStyle(element)[property as keyof CSSStyleDeclaration],
            property,
          ),
      ).toBe(expected);
    }
    expect(await ssr.locator("#client").textContent()).toBe("0");
    app.results.checks.push({
      name: `next-${app.mode}-${temperature}-ssr-without-javascript`,
      passed: true,
    });
  } finally {
    await ssr.close();
  }
  const evidence = await verifyPage(app, app.page, "rgb(44, 55, 66)");
  app.results.checks.push({
    name: `next-${app.mode}-${temperature}-production`,
    passed: true,
    evidence,
    javascriptSourceMap,
    cssSourceMap,
  });
  await app.stop();
}
