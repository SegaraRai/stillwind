import fs from "node:fs/promises";
import path from "node:path";

import { stillwind } from "@stillwind/integration/vite";
import { stylesheetLinks } from "stillwind/css";
import { build, createServer, preview } from "vite-plus";
import { expect } from "vite-plus/test";

import type { EmissionApplication } from "./emission-application.ts";

async function check(app: EmissionApplication, url: string, mode: string) {
  const { browser, record } = app.state;

  const classes: Record<string, string | null> = {};
  for (const first of ["a", "b"]) {
    const page = await browser.newPage();
    page.setDefaultTimeout(10000);
    const errors: string[] = [];
    page.on("pageerror", (error) => {
      errors.push(String(error));
      console.error(error);
    });
    page.on("console", (message) => {
      if (message.type() === "error") {
        console.error(message.text());
      }
    });
    await page.goto(`${url}?route=${first}`);
    await page.waitForFunction(() => typeof window.goto === "function" && window.sharedSheet);
    for (const route of [first, first === "a" ? "b" : "a"]) {
      await page.evaluate((route) => window.goto(route), route);
      await page.mouse.move(0, 0);
      await page.waitForFunction(
        (expected: string) =>
          getComputedStyle(document.querySelector<HTMLElement>("#target")!).marginLeft === expected,
        route === "a" ? "12px" : "20px",
      );
      classes[route] = await page.locator("#target").getAttribute("class");
      await page.locator("#target").hover();
      await page.waitForFunction(
        (expected: string) =>
          getComputedStyle(document.querySelector<HTMLElement>("#target")!).marginLeft === expected,
        route === "a" ? "80px" : "120px",
      );
    }
    await page.mouse.move(0, 0);
    const shadow = await page.evaluate(() => {
      const style = getComputedStyle(
        document
          .querySelector<HTMLElement>("#component")!
          .shadowRoot!.querySelector<HTMLElement>("#shadow")!,
      );
      return { margin: style.marginLeft, color: style.color };
    });
    expect(shadow).toEqual({ margin: "12px", color: "rgb(11, 22, 33)" });
    expect(errors).toEqual([]);
    await page.close();
  }
  record(`${mode}: reverse route order, hover, shadow closure and shared sheets`);
  return classes;
}

export async function expectLoadingOrder(app: EmissionApplication): Promise<void> {
  await check(app, app.server!.resolvedUrls!.local[0], "dev");
}

export async function expectCssOnlyUpdate(app: EmissionApplication): Promise<void> {
  const { browser, put, theme, record } = app.state;

  const page = await browser.newPage();
  await page.goto(app.server!.resolvedUrls!.local[0]);
  await page.waitForFunction(
    () =>
      getComputedStyle(document.querySelector<HTMLElement>("#target")!).color === "rgb(11, 22, 33)",
  );
  let reloads = 0;
  page.on("framenavigated", (frame) => {
    if (frame === page.mainFrame()) {
      reloads++;
    }
  });
  await page.evaluate(() => {
    window.originalStyle = document.querySelector<HTMLElement>("style[data-stillwind]")!;
  });
  try {
    await put("root.css", theme("rgb(44,55,66)"));
    await page.waitForFunction(
      () =>
        getComputedStyle(document.querySelector<HTMLElement>("#target")!).color ===
        "rgb(44, 55, 66)",
    );
    expect(reloads).toBe(0);
    expect(
      await page.evaluate(
        () =>
          window.originalStyle === document.querySelector<HTMLElement>("style[data-stillwind]")!,
      ),
    ).toBe(true);
    record("dev: CSS-only update retains style node and avoids reload");
  } finally {
    await put("root.css", theme("rgb(11,22,33)"));
    await page.close();
  }
}

export async function expectModuleRetraction(app: EmissionApplication): Promise<void> {
  const { browser, directory, put, record } = app.state;

  const page = await browser.newPage();
  await page.goto(app.server!.resolvedUrls!.local[0]);
  await page.waitForFunction(() => typeof window.goto === "function" && window.sharedSheet);
  try {
    await put(
      "added.ts",
      'import { style } from "stillwind/react";\n\nexport const props=style("m-7");\n',
    );
    await page.evaluate(`(async () => {
      const { props } = await import("/added.ts");
      document.querySelector("#target").className = props.className;
    })()`);
    await page.mouse.move(0, 0);
    await page.waitForFunction(
      () => getComputedStyle(document.querySelector<HTMLElement>("#target")!).marginLeft === "28px",
    );
    await fs.unlink(path.join(directory, "added.ts"));
    await page.waitForFunction(
      () => getComputedStyle(document.querySelector<HTMLElement>("#target")!).marginLeft !== "28px",
    );
    record("dev: module addition and deletion retract CSS");
  } finally {
    await fs.rm(path.join(directory, "added.ts"), { force: true });
    await page.close();
  }
}

export async function expectProduction(
  app: EmissionApplication,
  emission: "single" | "split",
  cssMinify: boolean,
): Promise<void> {
  const { browser, put, record, config } = app.state;

  const mode = `${emission}${cssMinify ? " minified" : ""}`;
  const result = await build({
    ...config(emission),
    build: { minify: false, cssMinify, sourcemap: true },
  });
  if (Array.isArray(result) || !("output" in result)) {
    expect.fail("Expected one build output");
  }
  const output = result.output;
  const css = output.filter((item) => item.type === "asset" && item.fileName.endsWith(".css"));
  expect(css.length > 0).toBeTruthy();
  if (emission === "single") {
    expect(css.length).toBe(1);
  } else {
    expect(
      css.length < 10,
      "split output packs adjacent rules into producer partitions",
    ).toBeTruthy();
  }
  const manifestAsset = output.find(
    (item) => item.type === "asset" && item.fileName === "stillwind-manifest.json",
  );
  if (!manifestAsset || manifestAsset.type !== "asset") {
    expect.fail("Missing manifest asset");
  }
  const manifest = JSON.parse(String(manifestAsset.source));
  expect(manifest.modules["a.ts"] && manifest.modules["b.ts"]).toBeTruthy();
  const code = output
    .filter((item) => item.type === "chunk")
    .map((item) => item.code)
    .join("\n");
  expect((code.match(/\.common-marker\s*\{/g) ?? []).length).toBe(1);
  app.server = await preview({
    ...config(emission),
    preview: {
      port: 0,
    },
  });
  const classes = await check(app, app.server!.resolvedUrls!.local[0], mode);
  for (const route of ["a", "b"]) {
    const links = stylesheetLinks(manifest, [`${route}.ts`]);
    await put(
      "dist/ssr.html",
      `${links.map((part) => `<link rel="stylesheet" href="${part.href}" data-stillwind-part="${part.id}" data-stillwind-order="${part.order}">`).join("")}<div id="target" class="${classes[route]}">SSR</div>`,
    );
    const page = await browser.newPage({ javaScriptEnabled: false });
    await page.goto(`${app.server!.resolvedUrls!.local[0]}ssr.html`);
    expect(
      await page.locator("#target").evaluate((element) => getComputedStyle(element).marginLeft),
    ).toBe(route === "a" ? "12px" : "20px");
    await page.close();
  }
  record(`${mode}: SSR applies selected CSS without JavaScript`);
  await new Promise<void>((resolve) => app.server!.httpServer!.close(() => resolve()));
  app.server = undefined;
}

export async function expectManualCss(app: EmissionApplication, mode: string): Promise<void> {
  const { browser, record, config } = app.state;

  const manual = {
    ...config(),
    plugins: [
      stillwind({
        css: "./root.css",
        types: false,
        automatic: false,
      }),
    ],
  };
  if (mode === "dev") {
    app.server = await createServer({
      ...manual,
      server: {
        port: 0,
      },
    });
    await app.server.listen();
  } else {
    const result = await build(manual);
    if (Array.isArray(result) || !("output" in result)) {
      expect.fail("Expected one build output");
    }
    const output = result.output;
    expect(!output.some((item) => item.fileName.endsWith(".css"))).toBeTruthy();
    app.server = await preview({
      ...manual,
      preview: {
        port: 0,
      },
    });
  }

  const page = await browser.newPage();
  await page.goto(app.server!.resolvedUrls!.local[0]);
  await page.waitForFunction(
    () =>
      window.sharedSheet &&
      getComputedStyle(
        document
          .querySelector<HTMLElement>("#component")!
          .shadowRoot!.querySelector<HTMLElement>("#shadow")!,
      ).marginLeft === "12px",
  );
  expect(
    await page.locator("#target").evaluate((element) => getComputedStyle(element).marginLeft),
  ).toBe("0px");
  expect(await page.locator("style[data-stillwind]").count()).toBe(0);
  await page.close();

  if ("close" in app.server) {
    await app.server.close();
  } else {
    await new Promise<void>((resolve) => app.server!.httpServer!.close(() => resolve()));
  }
  app.server = undefined;

  record(`Web Components ${mode}: explicit closure has no automatic document styles`);
}
