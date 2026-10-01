import assert from "node:assert/strict";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { stillwind } from "@stillwind/integration/vite";
import { chromium, type Browser, type Page } from "playwright";
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
const flow = await fs.readFile(path.join(dir, "flow.ts"), "utf8");
const flowSource = (nested: number) => flow.replace('"m-4"', '"m-' + nested + '"');
const writeSsrHtml = (html: string) =>
  put("ssr.html", `<div id="root">${html}</div><script type="module" src="/hydrate.ts"></script>`);
const ssrClass = (html: string) => {
  const match = html.match(/id="sample"[^>]*class="([^"]+)"/);
  assert.ok(match, `server html carries the sample class: ${html}`);
  return match[1];
};
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
const states: [{ enabled: boolean; nested: boolean }, string][] = [
  [{ enabled: false, nested: false }, "40px"],
  [{ enabled: true, nested: false }, "24px"],
  [{ enabled: true, nested: true }, "16px"],
];
async function applyState(page: Page, state: { enabled: boolean; nested: boolean }) {
  await page.evaluate((next) => window.setState(next), state);
}
let browser!: Browser;
let server:
  | Awaited<ReturnType<typeof createServer>>
  | Awaited<ReturnType<typeof preview>>
  | undefined;
try {
  browser = await chromium.launch({ channel: "chrome", headless: true });
  server = await createServer({ ...config(), server: { port: 0 } });
  await server.listen();
  const url = server.resolvedUrls!.local[0];

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
      assert.equal(
        await page.evaluate(() =>
          document.querySelector<HTMLElement>("#sample")!.getAttribute("aria-busy"),
        ),
        String(state.enabled),
      );
    }
    const classes = await page.evaluate(() => window.classes);
    assert.equal(classes.split(" ").length, 2);
    assert.deepEqual(errors, []);
    record(`${mode}-nested-conditional-states`);
    return page;
  }

  let page = await check(url, "dev");
  const before = await page.evaluate(() => window.classes);
  await put("flow.ts", flowSource(8));
  await page.waitForFunction(
    () => getComputedStyle(document.querySelector<HTMLElement>("#sample")!).marginTop === "32px",
    undefined,
    { timeout: 20000 },
  );
  record("dev-nested-conditional-content-reload");
  assert.notEqual(await page.evaluate(() => window.classes), before);
  // Restore the original branch content before the SSR/hydration check so the
  // server and client renders must agree on the same classes.
  await put("flow.ts", flowSource(4));
  await page.waitForFunction(
    () => getComputedStyle(document.querySelector<HTMLElement>("#sample")!).marginTop === "16px",
    undefined,
    { timeout: 20000 },
  );
  await page.close();
  const ssr = await server.ssrLoadModule("/ssr.ts");
  await writeSsrHtml(ssr.html);
  page = await browser.newPage();
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(String(error)));
  await page.goto(`${url}ssr.html`);
  await page.waitForFunction(
    () =>
      window.hydrated &&
      getComputedStyle(document.querySelector<HTMLElement>("#sample")!).marginTop === "16px",
  );
  assert.equal(await page.locator("#sample").getAttribute("class"), ssrClass(ssr.html));
  assert.deepEqual(errors, []);
  record("dev-SSR-hydration-class-equality");
  await page.close();
  await server.close();
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
        input: { main: path.join(dir, "index.html"), ssr: path.join(dir, "ssr.html") },
      },
    },
  });
  server = await preview({ ...config(), preview: { port: 0 } });
  const productionUrl = server.resolvedUrls!.local[0];
  page = await check(productionUrl, "production");
  await page.close();
  page = await browser.newPage();
  const productionErrors: string[] = [];
  page.on("pageerror", (error) => productionErrors.push(String(error)));
  await page.goto(`${productionUrl}ssr.html`);
  await page.waitForFunction(
    () =>
      window.hydrated &&
      getComputedStyle(document.querySelector<HTMLElement>("#sample")!).marginTop === "16px",
  );
  assert.equal(await page.locator("#sample").getAttribute("class"), ssrClass(productionSsr.html));
  assert.deepEqual(productionErrors, []);
  record("production-SSR-hydration-class-equality");
  await page.close();

  await fs.writeFile(
    path.join(process.env.STILLWIND_E2E_RESULTS || path.join(root, "results"), "conditional.json"),
    JSON.stringify(results, null, 2),
  );
  console.log(JSON.stringify(results, null, 2));
} finally {
  await server?.close();
  await browser?.close();
}

declare global {
  interface Window {
    setState: (state: { enabled: boolean; nested: boolean }) => void;
    classes: string;
    hydrated: boolean;
  }
}
