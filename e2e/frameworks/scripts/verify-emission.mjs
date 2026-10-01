import assert from "node:assert/strict";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { chromium } from "playwright";
import { build, createServer, preview } from "vite-plus";

import { stylesheetLinks } from "../../../packages/core/dist/css.js";
import { stillwind } from "../../../packages/integration/dist/vite/index.js";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const directory = await fs.mkdtemp(path.join(root, "output/emission-"));
const put = (name, source) => fs.writeFile(path.join(directory, name), source);
const core = path
  .relative(directory, path.resolve(root, "../../packages/core/index.css"))
  .replaceAll("\\", "/");
const theme = (color) =>
  `@import ${JSON.stringify(core)}; @utility paint{color:${color};} .common-marker{box-sizing:border-box;}`;
await put("root.css", theme("rgb(11,22,33)"));
await put(
  "index.html",
  '<link rel="icon" href="data:,"><div id="target" style="width:300px;height:300px">Target</div><div id="component"></div><script type="module" src="/main.ts"></script>',
);
await put(
  "base.ts",
  "import {style} from 'stillwind/react';export const base=style('m-10 hover:m-20');",
);
await put(
  "a.ts",
  "import {style} from 'stillwind/react';import {base} from './base';export const props=style(base,'m-3 paint');",
);
await put(
  "b.ts",
  "import {style} from 'stillwind/react';import {base} from './base';export const props=style(base,'m-5 hover:m-30');",
);
for (const name of ["one", "two"]) {
  await put(
    `${name}.ts`,
    "import {style} from 'stillwind/react';import {base} from './base';import {css,parts} from 'stillwind/module-css';export {css,parts};export const props=style(base,'m-3 paint');",
  );
}
await put(
  "main.ts",
  `
import * as one from './one'; import * as two from './two'; import {styleSheets} from 'stillwind/css';
const routes={a:()=>import('./a'),b:()=>import('./b')};
window.goto=async route=>{const {props}=await routes[route]();document.querySelector('#target').className=props.className;};
const root=document.querySelector('#component').shadowRoot??document.querySelector('#component').attachShadow({mode:'open'});
root.adoptedStyleSheets=styleSheets(one.parts,document);
const second=styleSheets(two.parts,document);
window.sharedSheet=one.parts.some((part,index)=>part.css.includes('.common-marker')&&second.includes(root.adoptedStyleSheets[index]));
root.innerHTML='<div id="shadow">Shadow</div>';root.querySelector('#shadow').className=one.props.className;
window.goto(new URL(location.href).searchParams.get('route')??'a');
if(import.meta.hot)import.meta.hot.accept();
`,
);
const config = (emission) => ({
  configFile: false,
  root: directory,
  logLevel: "silent",
  plugins: [stillwind({ css: path.join(directory, "root.css"), types: false, emission })],
});
const browser = await chromium.launch({ channel: "chrome", headless: true });
let server;
const checks = [];
const record = (name) => {
  checks.push(name);
  console.log(name);
};
try {
  const check = async (url, mode) => {
    const classes = {};
    for (const first of ["a", "b"]) {
      const page = await browser.newPage();
      page.setDefaultTimeout(10000);
      const errors = [];
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
      await page.waitForFunction(() => window.goto && window.sharedSheet);
      for (const route of [first, first === "a" ? "b" : "a"]) {
        await page.evaluate((route) => window.goto(route), route);
        await page.mouse.move(0, 0);
        await page.waitForFunction(
          (expected) => getComputedStyle(document.querySelector("#target")).marginLeft === expected,
          route === "a" ? "12px" : "20px",
        );
        classes[route] = await page.locator("#target").getAttribute("class");
        await page.locator("#target").hover();
        await page.waitForFunction(
          (expected) => getComputedStyle(document.querySelector("#target")).marginLeft === expected,
          route === "a" ? "80px" : "120px",
        );
      }
      await page.mouse.move(0, 0);
      const shadow = await page.evaluate(() => {
        const style = getComputedStyle(
          document.querySelector("#component").shadowRoot.querySelector("#shadow"),
        );
        return { margin: style.marginLeft, color: style.color };
      });
      assert.deepEqual(shadow, { margin: "12px", color: "rgb(11, 22, 33)" });
      assert.deepEqual(errors, []);
      await page.close();
    }
    record(`${mode}: reverse route order, hover, shadow closure and shared sheets`);
    return classes;
  };
  server = await createServer({ ...config(), server: { port: 0 }, css: { devSourcemap: true } });
  await server.listen();
  await check(server.resolvedUrls.local[0], "dev");
  const page = await browser.newPage();
  await page.goto(server.resolvedUrls.local[0]);
  await page.waitForFunction(
    () => getComputedStyle(document.querySelector("#target")).color === "rgb(11, 22, 33)",
  );
  let reloads = 0;
  page.on("framenavigated", (frame) => {
    if (frame === page.mainFrame()) {
      reloads++;
    }
  });
  await page.evaluate(() => {
    window.originalStyle = document.querySelector("style[data-stillwind]");
  });
  await put("root.css", theme("rgb(44,55,66)"));
  await page.waitForFunction(
    () => getComputedStyle(document.querySelector("#target")).color === "rgb(44, 55, 66)",
  );
  assert.equal(reloads, 0);
  assert.equal(
    await page.evaluate(
      () => window.originalStyle === document.querySelector("style[data-stillwind]"),
    ),
    true,
  );
  record("dev: CSS-only update retains style node and avoids reload");
  await put("added.ts", "import {style} from 'stillwind/react';export const props=style('m-7');");
  await page.evaluate(async () => {
    const { props } = await import("/added.ts");
    document.querySelector("#target").className = props.className;
  });
  await page.mouse.move(0, 0);
  await page.waitForFunction(
    () => getComputedStyle(document.querySelector("#target")).marginLeft === "28px",
  );
  await fs.unlink(path.join(directory, "added.ts"));
  await page.waitForFunction(
    () => getComputedStyle(document.querySelector("#target")).marginLeft !== "28px",
  );
  record("dev: module addition and deletion retract CSS");
  await page.close();
  await server.close();
  server = undefined;
  await put("root.css", theme("rgb(11,22,33)"));
  for (const [emission, cssMinify] of [
    ["single", false],
    ["split", false],
    ["single", true],
    ["split", true],
  ]) {
    const mode = `${emission}${cssMinify ? " minified" : ""}`;
    const result = await build({
      ...config(emission),
      build: { minify: false, cssMinify, sourcemap: true },
    });
    const output = result.output;
    const css = output.filter((item) => item.type === "asset" && item.fileName.endsWith(".css"));
    assert.ok(css.length > 0);
    if (emission === "single") {
      assert.equal(css.length, 1);
    } else {
      assert.ok(css.length < 10, "split output packs adjacent rules into producer partitions");
    }
    const manifest = JSON.parse(
      output.find((item) => item.fileName === "stillwind-manifest.json").source,
    );
    assert.ok(manifest.modules["a.ts"] && manifest.modules["b.ts"]);
    const code = output
      .filter((item) => item.type === "chunk")
      .map((item) => item.code)
      .join("\n");
    assert.equal((code.match(/\.common-marker\s*\{/g) ?? []).length, 1);
    server = await preview({ ...config(emission), preview: { port: 0 } });
    const classes = await check(server.resolvedUrls.local[0], mode);
    for (const route of ["a", "b"]) {
      const links = stylesheetLinks(manifest, [`${route}.ts`]);
      await put(
        "dist/ssr.html",
        `${links.map((part) => `<link rel="stylesheet" href="${part.href}" data-stillwind-part="${part.id}" data-stillwind-order="${part.order}">`).join("")}<div id="target" class="${classes[route]}">SSR</div>`,
      );
      const page = await browser.newPage({ javaScriptEnabled: false });
      await page.goto(`${server.resolvedUrls.local[0]}ssr.html`);
      assert.equal(
        await page.locator("#target").evaluate((element) => getComputedStyle(element).marginLeft),
        route === "a" ? "12px" : "20px",
      );
      await page.close();
    }
    record(`${mode}: SSR applies selected CSS without JavaScript`);
    await new Promise((resolve) => server.httpServer.close(resolve));
    server = undefined;
  }
  for (const mode of ["dev", "build"]) {
    const manual = {
      ...config(),
      plugins: [stillwind({ css: "./root.css", types: false, automatic: false })],
    };
    if (mode === "dev") {
      server = await createServer({ ...manual, server: { port: 0 } });
      await server.listen();
    } else {
      const output = (await build(manual)).output;
      assert.ok(!output.some((item) => item.fileName.endsWith(".css")));
      server = await preview({ ...manual, preview: { port: 0 } });
    }
    const page = await browser.newPage();
    await page.goto(server.resolvedUrls.local[0]);
    await page.waitForFunction(
      () =>
        window.sharedSheet &&
        getComputedStyle(document.querySelector("#component").shadowRoot.querySelector("#shadow"))
          .marginLeft === "12px",
    );
    assert.equal(
      await page.locator("#target").evaluate((element) => getComputedStyle(element).marginLeft),
      "0px",
    );
    assert.equal(await page.locator("style[data-stillwind]").count(), 0);
    await page.close();
    if (mode === "dev") {
      await server.close();
    } else {
      await new Promise((resolve) => server.httpServer.close(resolve));
    }
    server = undefined;
    record(`Web Components ${mode}: explicit closure has no automatic document styles`);
  }
} finally {
  await browser.close();
  if (server?.close) {
    await server.close();
  } else if (server) {
    await new Promise((resolve) => server.httpServer.close(resolve));
  }
}
console.log(JSON.stringify({ directory, checks }, null, 2));
