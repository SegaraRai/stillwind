// Acceptance probe for the real Stillwind Vite adapter (no surrogate macro).
import assert from "node:assert/strict";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { chromium } from "playwright";
import { createServer } from "vite-plus";

import { stillwind } from "../../../packages/integration/dist/vite/index.js";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const dir = path.join(root, "output", `stillwind-${Date.now()}`);
await fs.mkdir(dir, { recursive: true });
const put = (name, text) => fs.writeFile(path.join(dir, name), text);
const coreEntry = path
  .relative(dir, path.resolve(root, "../../packages/core/index.css"))
  .replaceAll("\\", "/");
const themeSource = (color) =>
  `@import ${JSON.stringify(coreEntry)}; @theme { --color-brand: ${color}; } .document-only { color: purple; }`;
await put("root.css", themeSource("rgb(11,22,33)"));
await put("index.html", '<div id="sample"></div><script type="module" src="/main.ts"></script>');
await put(
  "main.ts",
  'import { props } from "./sample"; document.querySelector("#sample").className = props.className ?? "";',
);
const source = 'import { style } from "stillwind/react"; export const props = style("bg-brand");';
await put("sample.ts", source);
const server = await createServer({
  configFile: false,
  root: dir,
  plugins: [stillwind({ css: path.join(dir, "root.css"), types: false })],
  logLevel: "silent",
  server: { port: 0 },
});
let browser;
const results = { checks: [] };
try {
  await server.listen();
  browser = await chromium.launch({ channel: "chrome", headless: true });
  const page = await browser.newPage();
  await page.goto(server.resolvedUrls.local[0]);
  const color = (expected) =>
    page.waitForFunction(
      (expected) =>
        getComputedStyle(document.querySelector("#sample")).backgroundColor === expected,
      expected,
      { timeout: 15000 },
    );
  await color("rgb(11, 22, 33)");
  results.checks.push({ name: "real-compiler-dev-css", passed: true });
  const before = await page.locator("#sample").getAttribute("class");
  await put("root.css", themeSource("rgb(44,55,66)"));
  await color("rgb(44, 55, 66)");
  assert.equal(await page.locator("#sample").getAttribute("class"), before);
  results.checks.push({ name: "css-only-edit-stable-class", passed: true });
  await put("sample.ts", "export const props = {};");
  await page.waitForFunction(() => document.querySelector("#sample").className === "", null, {
    timeout: 15000,
  });
  await color("rgba(0, 0, 0, 0)");
  results.checks.push({ name: "macro-removal", passed: true });
  await put("sample.ts", source);
  await color("rgb(44, 55, 66)");
  results.checks.push({ name: "macro-restoration", passed: true });
} finally {
  await browser?.close();
  await server.close();
  await fs.writeFile(path.join(root, "results/stillwind.json"), JSON.stringify(results, null, 2));
}
console.log(JSON.stringify(results, null, 2));
