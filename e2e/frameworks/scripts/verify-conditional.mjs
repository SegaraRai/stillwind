// Real-browser acceptance for nested conditional lowering through the Stillwind
// Vite adapter: development, production, SSR/hydration, and content reload.
import assert from "node:assert/strict";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { chromium } from "playwright";
import { build, createServer, preview } from "vite-plus";

import { stillwind } from "../../../packages/integration/dist/vite/index.js";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const dir = path.join(root, "output", `conditional-${Date.now()}`);
await fs.mkdir(dir, { recursive: true });
const put = (name, text) => fs.writeFile(path.join(dir, name), text);
const core = path
  .relative(dir, path.resolve(root, "../../packages/core/index.css"))
  .replaceAll("\\", "/");
await put("root.css", `@import ${JSON.stringify(core)};`);
await put("index.html", '<div id="app"></div><script type="module" src="/main.ts"></script>');

// `nested` picks the margin of the nested conditional branch.
const flowSource = (nested) =>
  [
    "import {createElement, useState} from 'react';",
    "import {style} from 'stillwind/react';",
    "export function App() {",
    "  const [state, setState] = useState({enabled: true, nested: true});",
    "  if (typeof window !== 'undefined') window.setState = setState;",
    "  const props = style(",
    "    'm-10',",
    `    state.enabled && (state.nested ? 'm-${nested}' : 'm-6'),`,
    "    { 'aria-busy': state.enabled },",
    "  );",
    "  if (typeof window !== 'undefined') window.classes = props.className;",
    "  return createElement('div', {id: 'sample', ...props, style: {width: 300, height: 300}}, 'sample');",
    "}",
  ].join("\n");
await put("flow.ts", flowSource(4));
await put(
  "main.ts",
  [
    "import {createElement} from 'react';",
    "import {createRoot} from 'react-dom/client';",
    "import {App} from './flow';",
    "createRoot(document.querySelector('#app')).render(createElement(App));",
  ].join("\n"),
);
await put(
  "ssr.ts",
  [
    "import {createElement} from 'react';",
    "import {renderToString} from 'react-dom/server';",
    "import {App} from './flow';",
    "export const html = renderToString(createElement(App));",
  ].join("\n"),
);
await put(
  "hydrate.ts",
  [
    "import {createElement} from 'react';",
    "import {hydrateRoot} from 'react-dom/client';",
    "import {App} from './flow';",
    "hydrateRoot(document.querySelector('#root'), createElement(App));",
    "window.hydrated = true;",
  ].join("\n"),
);
const writeSsrHtml = (html) =>
  put("ssr.html", `<div id="root">${html}</div><script type="module" src="/hydrate.ts"></script>`);
const ssrClass = (html) => {
  const match = html.match(/id="sample"[^>]*class="([^"]+)"/);
  assert.ok(match, `server html carries the sample class: ${html}`);
  return match[1];
};
const config = () => ({
  configFile: false,
  root: dir,
  plugins: [stillwind({ css: path.join(dir, "root.css"), types: false })],
  logLevel: "silent",
});
const results = { checks: [], directory: dir };
const record = (name) => {
  results.checks.push({ name, passed: true });
  console.log(name);
};
const states = [
  [{ enabled: false, nested: false }, "40px"],
  [{ enabled: true, nested: false }, "24px"],
  [{ enabled: true, nested: true }, "16px"],
];
async function applyState(page, state) {
  await page.evaluate((next) => window.setState(next), state);
}
let browser;
let server;
try {
  browser = await chromium.launch({ channel: "chrome", headless: true });
  server = await createServer({ ...config(), server: { port: 0 } });
  await server.listen();
  const url = server.resolvedUrls.local[0];

  async function check(target, mode) {
    const page = await browser.newPage();
    const errors = [];
    page.on("pageerror", (error) => errors.push(String(error)));
    await page.goto(target);
    await page.waitForFunction(
      () => typeof window.setState === "function" && document.querySelector("#sample"),
    );
    for (const [state, margin] of states) {
      await applyState(page, state);
      await page.waitForFunction(
        (value) => getComputedStyle(document.querySelector("#sample")).marginTop === value,
        margin,
        { timeout: 15000 },
      );
      assert.equal(
        await page.evaluate(() => document.querySelector("#sample").getAttribute("aria-busy")),
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
    () => getComputedStyle(document.querySelector("#sample")).marginTop === "32px",
    undefined,
    { timeout: 20000 },
  );
  record("dev-nested-conditional-content-reload");
  assert.notEqual(await page.evaluate(() => window.classes), before);
  // Restore the original branch content before the SSR/hydration check so the
  // server and client renders must agree on the same classes.
  await put("flow.ts", flowSource(4));
  await page.waitForFunction(
    () => getComputedStyle(document.querySelector("#sample")).marginTop === "16px",
    undefined,
    { timeout: 20000 },
  );
  await page.close();
  const ssr = await server.ssrLoadModule("/ssr.ts");
  await writeSsrHtml(ssr.html);
  page = await browser.newPage();
  const errors = [];
  page.on("pageerror", (error) => errors.push(String(error)));
  await page.goto(`${url}ssr.html`);
  await page.waitForFunction(
    () =>
      window.hydrated && getComputedStyle(document.querySelector("#sample")).marginTop === "16px",
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
  const productionUrl = server.resolvedUrls.local[0];
  page = await check(productionUrl, "production");
  await page.close();
  page = await browser.newPage();
  const productionErrors = [];
  page.on("pageerror", (error) => productionErrors.push(String(error)));
  await page.goto(`${productionUrl}ssr.html`);
  await page.waitForFunction(
    () =>
      window.hydrated && getComputedStyle(document.querySelector("#sample")).marginTop === "16px",
  );
  assert.equal(await page.locator("#sample").getAttribute("class"), ssrClass(productionSsr.html));
  assert.deepEqual(productionErrors, []);
  record("production-SSR-hydration-class-equality");
  await page.close();

  await fs.writeFile(
    path.join(root, "results", "conditional.json"),
    JSON.stringify(results, null, 2),
  );
  console.log(JSON.stringify(results, null, 2));
} finally {
  await server?.close();
  await browser?.close();
}
