import fs from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";

import type { ScenarioContext } from "@stillwind-internal/e2e-harness/scenario";
import { stillwind } from "@stillwind/integration/vite";
import { chromium, type Browser, type Page } from "playwright";
import { build, createServer, preview, type InlineConfig } from "vite";
import { afterAll, beforeAll, test, expect } from "vite-plus/test";

export default function register(context: ScenarioContext): void {
  let dir!: string;
  const write = async (name: string, text: string) => {
    const file = path.join(dir, name);
    await fs.mkdir(path.dirname(file), { recursive: true });
    await fs.writeFile(file, text);
  };
  const put = write;

  let flow!: string;
  const flowSource = (nested: number) => flow.replace('"m-4"', '"m-' + nested + '"');

  const writeSsrHtml = (html: string) =>
    put(
      "ssr.html",
      `<div id="root">${html}</div><script type="module" src="/hydrate.ts"></script>`,
    );

  const ssrClass = (html: string) => {
    const match = html.match(/id="sample"[^>]*class="([^"]+)"/);
    expect(match, `server html carries the sample class: ${html}`).toBeTruthy();
    return match![1];
  };

  const config = (): InlineConfig => ({
    configFile: false,
    root: dir,
    plugins: [
      stillwind({
        css: path.join(dir, "root.css"),
        types: false,
        emission: context.args[0] === "split" ? "split" : "single",
      }),
    ],
    logLevel: "silent",
  });

  const results = {
    checks: [] as {
      name: string;
      passed: boolean;
      [key: string]: unknown;
    }[],
    directory: dir,
  };
  const record = (name: string) => {
    results.checks.push({ name, passed: true });
    console.log(name);
  };

  const states: [{ enabled: boolean; nested: boolean }, string][] = [
    [{ enabled: false, nested: false }, "40px"],
    [{ enabled: true, nested: false }, "24px"],
    [{ enabled: true, nested: true }, "16px"],
  ];

  async function applyState(
    page: Page,
    state: {
      enabled: boolean;
      nested: boolean;
    },
  ) {
    await page.evaluate((next) => window.setState(next), state);
  }

  let browser!: Browser;
  let server:
    | Awaited<ReturnType<typeof createServer>>
    | Awaited<ReturnType<typeof preview>>
    | undefined;
  let url!: string;

  async function check(target: string, mode: string) {
    const page = await browser.newPage();
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(String(error)));
    await page.goto(target);
    await page.waitForFunction(
      () =>
        typeof window.setState === "function" && document.querySelector<HTMLElement>("#sample")!,
    );
    for (const [state, margin] of states) {
      await applyState(page, state);
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
    record(`${mode}-nested-conditional-states`);
    return page;
  }

  beforeAll(async () => {
    dir = context.directory;
    flow = await fs.readFile(path.join(dir, "flow.ts"), "utf8");
    browser = await chromium.launch({
      channel: "chrome",
      headless: true,
    });
    server = await createServer({
      ...config(),
      server: {
        port: 0,
      },
    });
    await server.listen();
    url = server.resolvedUrls!.local[0];
  }, 600_000);

  test("selects each nested condition branch and its aria value", async () => {
    const page = await check(url, "dev");
    await page.close();
  });

  test("reflects an edit inside a condition branch", async () => {
    const page = await check(url, "dev");
    const before = await page.evaluate(() => window.classes);
    await put("flow.ts", flowSource(8));
    await page.waitForFunction(
      () => getComputedStyle(document.querySelector<HTMLElement>("#sample")!).marginTop === "32px",
      undefined,
      { timeout: 20000 },
    );
    expect(await page.evaluate(() => window.classes)).not.toBe(before);
    record("dev-nested-conditional-content-reload");
    await put("flow.ts", flowSource(4));
    await page.close();
  });

  test("preserves conditional classes during SSR hydration", async () => {
    await put("flow.ts", flowSource(4));
    const ssr = await (server! as Awaited<ReturnType<typeof createServer>>).ssrLoadModule(
      "/ssr.ts",
    );
    await writeSsrHtml(ssr.html);
    const page = await browser.newPage();
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(String(error)));
    await page.goto(`${url}ssr.html`);
    await page.waitForFunction(
      () =>
        window.hydrated &&
        getComputedStyle(document.querySelector<HTMLElement>("#sample")!).marginTop === "16px",
    );
    expect(await page.locator("#sample").getAttribute("class")).toBe(ssrClass(ssr.html));
    expect(errors).toEqual([]);
    record("dev-SSR-hydration-class-equality");
    await page.close();
  });

  test("production: selects condition branches and preserves SSR classes", async () => {
    if (server && "close" in server) {
      await server.close();
    }
    server = undefined;
    await put("flow.ts", flowSource(4));
    await build({
      ...config(),
      build: { ssr: path.join(dir, "ssr.ts"), outDir: "dist-server", minify: false },
    });
    const productionSsr = await import(pathToFileURL(path.join(dir, "dist-server/ssr.js")).href);
    await writeSsrHtml(productionSsr.html);
    await build({
      ...config(),
      build: {
        minify: false,
        cssMinify: false,
        rolldownOptions: {
          input: {
            main: path.join(dir, "index.html"),
            ssr: path.join(dir, "ssr.html"),
          },
        },
      },
    });
    server = await preview({
      ...config(),
      preview: {
        port: 0,
      },
    });
    const productionUrl = server.resolvedUrls!.local[0];
    const branchPage = await check(productionUrl, "production");
    await branchPage.close();
    const page = await browser.newPage();
    const productionErrors: string[] = [];
    page.on("pageerror", (error) => productionErrors.push(String(error)));
    await page.goto(`${productionUrl}ssr.html`);
    await page.waitForFunction(
      () =>
        window.hydrated &&
        getComputedStyle(document.querySelector<HTMLElement>("#sample")!).marginTop === "16px",
    );
    expect(await page.locator("#sample").getAttribute("class")).toBe(ssrClass(productionSsr.html));
    expect(productionErrors).toEqual([]);
    record("production-SSR-hydration-class-equality");
    await page.close();
  });

  afterAll(async () => {
    await server?.close();
    await browser?.close();
    await fs.writeFile(
      path.join(context.resultsDirectory, "conditional.json"),
      JSON.stringify(results, null, 2),
    );
  });
}

declare global {
  interface Window {
    setState: (state: { enabled: boolean; nested: boolean }) => void;
    classes: string;
    hydrated: boolean;
  }
}
