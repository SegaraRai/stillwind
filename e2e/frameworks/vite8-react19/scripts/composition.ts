import assert from "node:assert/strict";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { stillwind } from "@stillwind/integration/vite";
import { chromium, type Browser } from "playwright";
import { build, createServer, preview, type InlineConfig } from "vite-plus";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const dir = process.env.STILLWIND_E2E_WORKDIR!;
if (!dir) {
  throw new Error("Use the fixture e2e task to create its work directory");
}
await fs.mkdir(path.join(process.env.STILLWIND_E2E_RESULTS || path.join(root, "results")), {
  recursive: true,
});
const write = async (name: string, text: string) => {
  const file = path.join(dir, name);
  await fs.mkdir(path.dirname(file), { recursive: true });
  await fs.writeFile(file, text);
};
const put = write;
const baseSource = (value: number) =>
  'import { style } from "stillwind/react";export const base=style(\'m-' + value + " hover:m-20');";
const routeSource = (base: string, delta: number) =>
  'import { style } from "stillwind/react";import {' +
  base +
  "} from './shared';export const props=style(" +
  base +
  ",'m-" +
  delta +
  "');";
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
      types: false,
      emission: process.argv[2] === "split" ? "split" : "single",
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
try {
  browser = await chromium.launch({ channel: "chrome", headless: true });
  server = await createServer({ ...config(), server: { port: 0 } });
  await server.listen();
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
      assert.deepEqual(errors, []);
      await page.close();
    }
  }
  const url = server.resolvedUrls!.local[0];
  await check(url, "dev");
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
    assert.equal(await page.locator("#sample").getAttribute("class"), props.className);
    assert.deepEqual(errors, []);
    record(`${mode}-SSR-hydration-identities-and-CSS`);
    await page.close();
  }
  const ssr = await server.ssrLoadModule("/ssr.ts");
  await writeSsrHtml(ssr.html);
  await checkHydration(url, ssr.props, "dev");
  const page = await browser.newPage();
  await page.goto(`${url}?route=a`);
  await page.waitForFunction(() => window.current === "a");
  let reloads = 0;
  page.on("framenavigated", (frame) => {
    if (frame === page.mainFrame()) {
      reloads++;
    }
  });
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
    () => getComputedStyle(document.querySelector<HTMLElement>("#sample")!).marginTop === "12px",
  );
  record("dev-base-content-reload-preserves-order");
  const previous = reloads;
  await put("a.ts", routeSource("diamond", 7));
  await page.waitForFunction(
    () => getComputedStyle(document.querySelector<HTMLElement>("#sample")!).marginTop === "28px",
  );
  assert.ok(reloads > previous);
  record("dev-topology-reload");
  await page.close();
  await server.close();
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
  server = await preview({ ...config(), preview: { port: 0 } });
  await check(server.resolvedUrls!.local[0], "production");
  await checkHydration(server.resolvedUrls!.local[0], productionSsr.props, "production");
} finally {
  await browser?.close();
  if (server && "close" in server) {
    await server.close();
  } else {
    await new Promise<void>((resolve) => server?.httpServer?.close(() => resolve()) ?? resolve());
  }
  await fs.writeFile(
    path.join(process.env.STILLWIND_E2E_RESULTS || path.join(root, "results"), "composition.json"),
    JSON.stringify(results, null, 2),
  );
}
console.log(JSON.stringify(results, null, 2));

declare global {
  interface Window {
    goto: (route: string) => Promise<void>;
    hydratedProps: Record<string, string>;
    current: string;
    hydrated: boolean;
  }
}
