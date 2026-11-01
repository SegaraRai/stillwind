import path from "node:path";
import { pathToFileURL } from "node:url";

import { build, createServer, preview } from "vite";
import { expect } from "vite-plus/test";

import type { ConditionalApplication } from "./conditional-application.ts";

async function check(app: ConditionalApplication, target: string, mode: string) {
  const page = await app.browser.newPage();
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(String(error)));
  await page.goto(target);
  await page.waitForFunction(
    () => typeof window.setState === "function" && document.querySelector<HTMLElement>("#sample")!,
  );
  for (const [state, margin] of app.states) {
    await app.applyState(page, state);
    await page.waitForFunction(
      (value: string) =>
        getComputedStyle(document.querySelector<HTMLElement>("#sample")!).marginTop === value,
      margin,
      { timeout: 15000 },
    );
    expect(
      await page.evaluate(() =>
        document.querySelector<HTMLElement>("#sample")!.getAttribute("aria-busy"),
      ),
    ).toBe(String(state.enabled));
  }
  const classes = await page.evaluate(() => window.classes);
  expect(classes.split(" ").length).toBe(2);
  expect(errors).toEqual([]);
  app.record(`${mode}-nested-conditional-states`);
  return page;
}

export async function expectConditionBranches(app: ConditionalApplication): Promise<void> {
  const page = await check(app, app.url, "dev");
  await page.close();
}

export async function expectBranchEdit(app: ConditionalApplication): Promise<void> {
  const page = await check(app, app.url, "dev");
  const before = await page.evaluate(() => window.classes);
  await app.put("flow.ts", app.flowSource(8));
  // The edit reloads the page, and `#sample` is absent until the application renders again.
  await page.waitForFunction(
    () => {
      const sample = document.querySelector<HTMLElement>("#sample");
      return sample !== null && getComputedStyle(sample).marginTop === "32px";
    },
    undefined,
    { timeout: 20000 },
  );
  expect(await page.evaluate(() => window.classes)).not.toBe(before);
  app.record("dev-nested-conditional-content-reload");
  await app.put("flow.ts", app.flowSource(4));
  await page.close();
}

export async function expectHydration(app: ConditionalApplication): Promise<void> {
  await app.put("flow.ts", app.flowSource(4));
  const ssr = await (app.server! as Awaited<ReturnType<typeof createServer>>).ssrLoadModule(
    "/ssr.ts",
  );
  await app.writeSsrHtml(ssr.html);
  const page = await app.browser.newPage();
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(String(error)));
  await page.goto(`${app.url}ssr.html`);
  await page.waitForFunction(
    () =>
      window.hydrated &&
      getComputedStyle(document.querySelector<HTMLElement>("#sample")!).marginTop === "16px",
  );
  expect(await page.locator("#sample").getAttribute("class")).toBe(app.ssrClass(ssr.html));
  expect(errors).toEqual([]);
  app.record("dev-SSR-hydration-class-equality");
  await page.close();
}

export async function expectProduction(app: ConditionalApplication): Promise<void> {
  if (app.server && "close" in app.server) {
    await app.server.close();
  }
  app.server = undefined;
  await app.put("flow.ts", app.flowSource(4));
  await build({
    ...app.config(),
    build: {
      ssr: path.join(app.dir, "ssr.ts"),
      outDir: "dist-server",
      minify: false,
    },
  });
  const productionSsr = await import(pathToFileURL(path.join(app.dir, "dist-server/ssr.js")).href);
  await app.writeSsrHtml(productionSsr.html);
  await build({
    ...app.config(),
    build: {
      minify: false,
      cssMinify: false,
      rolldownOptions: {
        input: {
          main: path.join(app.dir, "index.html"),
          ssr: path.join(app.dir, "ssr.html"),
        },
      },
    },
  });
  app.server = await preview({
    ...app.config(),
    preview: {
      port: 0,
    },
  });
  const productionUrl = app.server.resolvedUrls!.local[0];
  const branchPage = await check(app, productionUrl, "production");
  await branchPage.close();
  const page = await app.browser.newPage();
  const productionErrors: string[] = [];
  page.on("pageerror", (error) => productionErrors.push(String(error)));
  await page.goto(`${productionUrl}ssr.html`);
  await page.waitForFunction(
    () =>
      window.hydrated &&
      getComputedStyle(document.querySelector<HTMLElement>("#sample")!).marginTop === "16px",
  );
  expect(await page.locator("#sample").getAttribute("class")).toBe(
    app.ssrClass(productionSsr.html),
  );
  expect(productionErrors).toEqual([]);
  app.record("production-SSR-hydration-class-equality");
  await page.close();
}
