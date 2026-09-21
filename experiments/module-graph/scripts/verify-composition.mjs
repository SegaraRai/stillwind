// Real-browser acceptance for composition through the Stillwind Vite adapter.
import assert from "node:assert/strict";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { chromium } from "playwright";
import { build, createServer, preview } from "vite-plus";

import { stillwind } from "../../../packages/integration/src/vite/index.ts";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const dir = path.join(root, "output", `composition-${Date.now()}`);
await fs.mkdir(dir, { recursive: true });
const put = (name, text) => fs.writeFile(path.join(dir, name), text);
const core = path
  .relative(dir, path.resolve(root, "../../packages/core/index.css"))
  .replaceAll("\\", "/");
const stylesheet = `@import ${JSON.stringify(core)};`;
await put("root.css", stylesheet);
await put(
  "index.html",
  '<div id="sample" style="width:300px;height:300px">sample</div><script type="module" src="/main.ts"></script>',
);
const baseSource = (value) =>
  `import {style} from 'stillwind/react';export const base=style('m-${value} hover:m-20');`;
await put("base.ts", baseSource(10));
await put(
  "shared.ts",
  `import {style} from 'stillwind/react';import {base} from './base';export const middle=style(base,'m-4');export const back=style(middle,'m-10');export const left=style(base,'m-6');export const right=style(base,'m-8');export const diamond=style(left,right,'m-2');`,
);
const routeSource = (base, delta) =>
  `import {style} from 'stillwind/react';import {${base}} from './shared';export const props=style(${base},'m-${delta}');`;
await put("a.ts", routeSource("back", 3));
await put("b.ts", routeSource("diamond", "5 hover:m-30"));
await put(
  "main.ts",
  `const routes={a:()=>import('./a'),b:()=>import('./b')};window.goto=async(name)=>{const {props}=await routes[name]();document.querySelector('#sample').className=props.className;window.current=name;};window.goto(new URL(location.href).searchParams.get('route')??'a');`,
);
await put(
  "ssr.ts",
  `import {createElement} from 'react';import {renderToString} from 'react-dom/server';import {props} from './a';export {props};export const html=renderToString(createElement('div',{id:'sample',...props,style:{width:300,height:300}},'sample'));`,
);
await put(
  "hydrate.ts",
  `import {createElement} from 'react';import {hydrateRoot} from 'react-dom/client';import {props} from './a';hydrateRoot(document.querySelector('#root'),createElement('div',{id:'sample',...props,style:{width:300,height:300}},'sample'));window.hydratedProps=props;`,
);
const writeSsrHtml = (html) =>
  put("ssr.html", `<div id="root">${html}</div><script type="module" src="/hydrate.ts"></script>`);
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
let browser;
let server;
try {
  browser = await chromium.launch({ channel: "chrome", headless: true });
  server = await createServer({ ...config(), server: { port: 0 } });
  await server.listen();
  async function check(url, mode) {
    for (const first of ["a", "b"]) {
      const page = await browser.newPage();
      const errors = [];
      page.on("pageerror", (error) => errors.push(String(error)));
      page.on("response", async (response) => {
        if (response.status() >= 400)
          console.error(
            response.status(),
            response.url().split("?")[0],
            (await response.text()).slice(0, 1500),
          );
      });
      await page.goto(`${url}?route=${first}`);
      for (const name of [first, first === "a" ? "b" : "a"]) {
        await page.waitForFunction(() => typeof window.goto === "function");
        await page.evaluate((name) => window.goto(name), name);
        const expected = name === "a" ? "12px" : "20px";
        await page.mouse.move(0, 0);
        await page.waitForFunction(
          (expected) => getComputedStyle(document.querySelector("#sample")).marginTop === expected,
          expected,
          { timeout: 15000 },
        );
        await page.locator("#sample").hover();
        await page.waitForFunction(
          (expected) => getComputedStyle(document.querySelector("#sample")).marginTop === expected,
          name === "a" ? "80px" : "120px",
          { timeout: 15000 },
        );
        record(`${mode}-${first}-then-${name}-normal-and-hover`);
      }
      assert.deepEqual(errors, []);
      await page.close();
    }
  }
  const url = server.resolvedUrls.local[0];
  await check(url, "dev");
  async function checkHydration(url, props, mode) {
    const page = await browser.newPage();
    const errors = [];
    page.on("pageerror", (error) => errors.push(String(error)));
    page.on("console", (message) => {
      if (message.type() === "error") errors.push(message.text());
    });
    await page.goto(`${url}ssr.html`);
    await page.waitForFunction(
      () =>
        window.hydratedProps &&
        getComputedStyle(document.querySelector("#sample")).marginTop === "12px",
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
    if (frame === page.mainFrame()) reloads++;
  });
  await put("base.ts", baseSource(12));
  await page.waitForFunction(() => window.current === "a");
  await new Promise((resolve, reject) => {
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
    () => getComputedStyle(document.querySelector("#sample")).marginTop === "12px",
  );
  record("dev-base-content-reload-preserves-order");
  const previous = reloads;
  await put("a.ts", routeSource("diamond", 7));
  await page.waitForFunction(
    () => getComputedStyle(document.querySelector("#sample")).marginTop === "28px",
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
  await check(server.resolvedUrls.local[0], "production");
  await checkHydration(server.resolvedUrls.local[0], productionSsr.props, "production");
} finally {
  await browser?.close();
  if (server?.close) await server.close();
  else await new Promise((resolve) => server?.httpServer?.close(resolve) ?? resolve());
  await fs.writeFile(path.join(root, "results/composition.json"), JSON.stringify(results, null, 2));
}
console.log(JSON.stringify(results, null, 2));
