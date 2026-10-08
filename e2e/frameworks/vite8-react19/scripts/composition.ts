import fs from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";

import type { ScenarioContext } from "@stillwind-internal/e2e-harness/scenario";
import { stillwind } from "@stillwind/integration/vite";
import { chromium, type Browser } from "playwright";
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

  const baseSource = (value: number) =>
    'import { style } from "stillwind/react";\nexport const base = style("m-' +
    value +
    ' hover:m-20");';
  const routeSource = (base: string, delta: number) =>
    'import { style } from "stillwind/react";\nimport {' +
    base +
    '} from "./shared";\nexport const props = style(' +
    base +
    ', "m-' +
    delta +
    '");\n';

  const writeSsrHtml = (html: string) =>
    put(
      "ssr.html",
      '<div id="root">' + html + '</div><script type="module" src="/hydrate.ts"></script>',
    );

  const config = (): InlineConfig => ({
    configFile: false,
    root: dir,
    plugins: [
      stillwind({
        css: path.join(dir, "root.css"),
        emission: context.args[0] === "split" ? "split" : "single",
      }),
    ],
    logLevel: "silent",
  });

  const results = {
    checks: [] as { name: string; passed: boolean; [key: string]: unknown }[],
    directory: dir,
  };
  const record = (name: string) => {
    results.checks.push({ name, passed: true });
    console.log(name);
  };

  let browser!: Browser;
  let server:
    | Awaited<ReturnType<typeof createServer>>
    | Awaited<ReturnType<typeof preview>>
    | undefined;
  let url!: string;

  async function check(url: string, mode: string) {
    for (const first of ["a", "b"]) {
      const page = await browser.newPage();
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
            getComputedStyle(document.querySelector<HTMLElement>("#sample")!).marginTop ===
            expected,
          expected,
          { timeout: 15000 },
        );
        await page.locator("#sample").hover();
        await page.waitForFunction(
          (expected: string) =>
            getComputedStyle(document.querySelector<HTMLElement>("#sample")!).marginTop ===
            expected,
          name === "a" ? "80px" : "120px",
          { timeout: 15000 },
        );
        record(`${mode}-${first}-then-${name}-normal-and-hover`);
      }
      expect(errors).toEqual([]);
      await page.close();
    }
  }

  async function checkHydration(url: string, props: Record<string, string>, mode: string) {
    const page = await browser.newPage();
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
    record(`${mode}-SSR-hydration-identities-and-CSS`);
    await page.close();
  }

  beforeAll(async () => {
    dir = context.directory;

    browser = await chromium.launch({ channel: "chrome", headless: true });
    server = await createServer({
      ...config(),
      server: {
        port: 0,
      },
    });
    await server.listen();
    url = server.resolvedUrls!.local[0];
  }, 600_000);

  test("resolves composed CSS and hover rules in both lazy loading orders", async () => {
    await check(url, "dev");
  });

  test("preserves SSR classes and CSS during hydration", async () => {
    const ssr = await (server! as Awaited<ReturnType<typeof createServer>>).ssrLoadModule(
      "/ssr.ts",
    );
    await writeSsrHtml(ssr.html);
    await checkHydration(url, ssr.props, "dev");
  });

  test("updates a composed dependency without changing precedence", async () => {
    const page = await browser.newPage();
    await page.goto(`${url}?route=a`);
    await page.waitForFunction(() => window.current === "a");
    let reloads = 0;
    page.on("framenavigated", (frame) => {
      if (frame === page.mainFrame()) {
        reloads++;
      }
    });
    try {
      await put("base.ts", baseSource(12));
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
        () =>
          getComputedStyle(document.querySelector<HTMLElement>("#sample")!).marginTop === "12px",
      );
      record("dev-base-content-reload-preserves-order");
    } finally {
      await put("base.ts", baseSource(10));
      await page.close();
    }
  });

  test("updates CSS after changing the composition graph", async () => {
    const page = await browser.newPage();
    await page.goto(`${url}?route=a`);
    await page.waitForFunction(() => window.current === "a");
    let reloads = 0;
    page.on("framenavigated", (frame) => {
      if (frame === page.mainFrame()) {
        reloads++;
      }
    });
    try {
      await put("a.ts", routeSource("diamond", 7));
      await page.waitForFunction(
        () =>
          getComputedStyle(document.querySelector<HTMLElement>("#sample")!).marginTop === "28px",
      );
      expect(reloads > 0).toBeTruthy();
      record("dev-topology-reload");
    } finally {
      await put("a.ts", routeSource("back", 3));
      await page.close();
    }
  });

  test("production: preserves lazy loading order and SSR class identity", async () => {
    if (server && "close" in server) {
      await server.close();
    }
    server = undefined;
    await put("base.ts", baseSource(10));
    await put("a.ts", routeSource("back", 3));
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
          input: { main: path.join(dir, "index.html"), ssr: path.join(dir, "ssr.html") },
        },
      },
    });
    server = await preview({
      ...config(),
      preview: {
        port: 0,
      },
    });
    await check(server.resolvedUrls!.local[0], "production");
    await checkHydration(server.resolvedUrls!.local[0], productionSsr.props, "production");
  });

  afterAll(async () => {
    await browser?.close();
    if (server && "close" in server) {
      await server.close();
    } else {
      await new Promise<void>((resolve) => server?.httpServer?.close(() => resolve()) ?? resolve());
    }
    await fs.writeFile(
      path.join(context.resultsDirectory, "composition.json"),
      JSON.stringify(results, null, 2),
    );
  });
}

declare global {
  interface Window {
    goto: (route: string) => Promise<void>;
    hydratedProps: Record<string, string>;
    current: string;
    hydrated: boolean;
  }
}
