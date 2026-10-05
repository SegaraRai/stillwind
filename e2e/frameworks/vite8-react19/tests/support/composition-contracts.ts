import path from "node:path";
import { pathToFileURL } from "node:url";

import { build, createServer, preview } from "vite";
import { expect } from "vite-plus/test";

import type { CompositionApplication } from "./composition-application.ts";

async function check(app: CompositionApplication, url: string, mode: string) {
  for (const first of ["a", "b"]) {
    const page = await app.browser.newPage();
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(String(error)));
    page.on("response", async (response) => {
      if (response.status() >= 400) {
        console.error(
          response.status(),
          response.url().split("?")[0],
          (await response.text()).slice(0, 1500),
        );
      }
    });
    await page.goto(`${url}?route=${first}`);
    for (const name of [first, first === "a" ? "b" : "a"]) {
      await page.waitForFunction(() => typeof window.goto === "function");
      await page.evaluate((name: string) => window.goto(name), name);
      const expected = name === "a" ? "12px" : "20px";
      await page.mouse.move(0, 0);
      await page.waitForFunction(
        (expected: string) =>
          getComputedStyle(document.querySelector<HTMLElement>("#sample")!).marginTop === expected,
        expected,
        { timeout: 15000 },
      );
      await page.locator("#sample").hover();
      await page.waitForFunction(
        (expected: string) =>
          getComputedStyle(document.querySelector<HTMLElement>("#sample")!).marginTop === expected,
        name === "a" ? "80px" : "120px",
        { timeout: 15000 },
      );
      app.record(`${mode}-${first}-then-${name}-normal-and-hover`);
    }
    expect(errors).toEqual([]);
    await page.close();
  }
}

async function checkHydration(
  app: CompositionApplication,
  url: string,
  props: Record<string, string>,
  mode: string,
) {
  const page = await app.browser.newPage();
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(String(error)));
  page.on("console", (message) => {
    if (message.type() === "error") {
      errors.push(message.text());
    }
  });
  await page.goto(`${url}ssr.html`);
  await page.waitForFunction(
    () =>
      window.hydratedProps &&
      getComputedStyle(document.querySelector<HTMLElement>("#sample")!).marginTop === "12px",
  );
  expect(await page.locator("#sample").getAttribute("class")).toBe(props.className);
  expect(errors).toEqual([]);
  app.record(`${mode}-SSR-hydration-identities-and-CSS`);
  await page.close();
}

export async function expectLoadingOrder(app: CompositionApplication): Promise<void> {
  await check(app, app.url, "dev");
}

export async function expectHydration(app: CompositionApplication): Promise<void> {
  const ssr = await (app.server! as Awaited<ReturnType<typeof createServer>>).ssrLoadModule(
    "/ssr.ts",
  );
  await app.writeSsrHtml(ssr.html);
  await checkHydration(app, app.url, ssr.props, "dev");
}

export async function expectDependencyEdit(app: CompositionApplication): Promise<void> {
  const page = await app.browser.newPage();
  await page.goto(`${app.url}?route=a`);
  await page.waitForFunction(() => window.current === "a");
  let reloads = 0;
  page.on("framenavigated", (frame) => {
    if (frame === page.mainFrame()) {
      reloads++;
    }
  });
  try {
    await app.put("base.ts", app.baseSource(12));
    await page.waitForFunction(() => window.current === "a");
    await new Promise<void>((resolve, reject) => {
      const start = Date.now();
      const timer = setInterval(() => {
        if (reloads > 0) {
          clearInterval(timer);
          resolve();
        } else if (Date.now() - start > 15000) {
          clearInterval(timer);
          reject(new Error("base edit did not reload"));
        }
      }, 50);
    });
    await page.waitForFunction(
      () => getComputedStyle(document.querySelector<HTMLElement>("#sample")!).marginTop === "12px",
    );
    app.record("dev-base-content-reload-preserves-order");
  } finally {
    await app.put("base.ts", app.baseSource(10));
    await page.close();
  }
}

export async function expectTopologyEdit(app: CompositionApplication): Promise<void> {
  const page = await app.browser.newPage();
  await page.goto(`${app.url}?route=a`);
  await page.waitForFunction(() => window.current === "a");
  let reloads = 0;
  page.on("framenavigated", (frame) => {
    if (frame === page.mainFrame()) {
      reloads++;
    }
  });
  try {
    await app.put("a.ts", app.routeSource("diamond", 7));
    await page.waitForFunction(
      () => getComputedStyle(document.querySelector<HTMLElement>("#sample")!).marginTop === "28px",
    );
    expect(reloads > 0).toBeTruthy();
    app.record("dev-topology-reload");
  } finally {
    await app.put("a.ts", app.routeSource("back", 3));
    await page.close();
  }
}

export async function expectProduction(app: CompositionApplication): Promise<void> {
  if (app.server && "close" in app.server) {
    await app.server.close();
  }
  app.server = undefined;
  await app.put("base.ts", app.baseSource(10));
  await app.put("a.ts", app.routeSource("back", 3));
  await build({
    ...app.config(),
    build: { ssr: path.join(app.dir, "ssr.ts"), outDir: "dist-server", minify: false },
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
  await check(app, app.server.resolvedUrls!.local[0], "production");
  await checkHydration(app, app.server.resolvedUrls!.local[0], productionSsr.props, "production");
}
