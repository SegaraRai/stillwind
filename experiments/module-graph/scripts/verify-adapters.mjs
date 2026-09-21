import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { chromium } from "playwright";

import { verifyMacroSourceMap } from "./verify-source-map.mjs";

const require = createRequire(import.meta.url);
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const repo = path.resolve(root, "../..");
const mode = process.argv[2] ?? "turbopack";
assert(["turbopack", "webpack"].includes(mode));
const port = mode === "webpack" ? 43175 : 43174;
const url = `http://localhost:${port}`;
const dir = path.join(root, "output", `adapters-${mode}-${Date.now()}`);
const results = { mode, next: require("next/package.json").version, checks: [] };
const browserErrors = [];
const write = async (name, text) => {
  const file = path.join(dir, name);
  await fs.mkdir(path.dirname(file), { recursive: true });
  await fs.writeFile(file, text);
};
const delay = (ms) => new Promise((done) => setTimeout(done, ms).unref());
const theme = (color) =>
  `@import ${JSON.stringify(path.relative(dir, path.join(repo, "packages/core/index.css")).replaceAll("\\", "/"))}; @theme {--color-brand:${color};} @utility client {color:rgb(70,80,90)} @utility lazy {color:rgb(90,80,70)}`;
await write("package.json", JSON.stringify({ private: true }));
await write("root.css", theme("rgb(11,22,33)"));
await write(
  "next.config.js",
  `const {withStillwind}=require(${JSON.stringify(path.join(repo, "packages/integration/src/next/index.cjs"))}); module.exports=withStillwind({productionBrowserSourceMaps:true,turbopack:{root:${JSON.stringify(repo)}}},{css:${JSON.stringify(path.join(dir, "root.css"))},root:${JSON.stringify(dir)}});`,
);
await write(
  "app/layout.jsx",
  "export default function Layout({children}){return <html><body>{children}</body></html>}",
);
await write(
  "app/page.jsx",
  `import {style} from 'stillwind/react';\nimport Client from './client';\nimport Link from 'next/link';\nexport default function Page(){return <><p id="server" {...style('text-brand')}>Server</p><Client/><Link id="link" href="/lazy" prefetch={true}>Lazy</Link></>}`,
);
await write(
  "app/client.jsx",
  `'use client';\nimport {style} from 'stillwind/react';\nimport {useState} from 'react';\nexport default function Client(){const [n,setN]=useState(0);return <button id="client" {...style('client')} onClick={()=>setN(n+1)}>{n}</button>}`,
);
await write(
  "app/lazy/page.jsx",
  `import {style} from 'stillwind/react';\nimport Link from 'next/link';\nexport default function Page(){return <><p id="lazy" {...style('lazy')}>Lazy</p><Link id="back" href="/">Back</Link></>}`,
);
let browser;
let proc;
function start(args) {
  const child = spawn(process.execPath, [require.resolve("next/dist/bin/next"), ...args], {
    cwd: dir,
    windowsHide: true,
    env: {
      ...process.env,
      NODE_ENV: args[0] === "dev" ? "development" : "production",
      NEXT_TELEMETRY_DISABLED: "1",
    },
    stdio: ["ignore", "pipe", "pipe"],
  });
  let logs = "";
  child.stdout.on("data", (data) => {
    logs += data;
    process.stdout.write(data);
  });
  child.stderr.on("data", (data) => {
    logs += data;
    process.stderr.write(data);
  });
  return {
    child,
    logs: () => logs,
    done: new Promise((done) => child.on("exit", (code) => done(code))),
  };
}
async function ready() {
  for (let i = 0; i < 100; i++) {
    if (proc.child.exitCode !== null) {
      throw new Error(proc.logs());
    }
    try {
      if ((await fetch(url, { signal: AbortSignal.timeout(2000) })).ok) {
        return;
      }
    } catch {}
    await delay(300);
  }
  throw new Error(proc.logs());
}
async function verifyPage(page, color) {
  const prefetch = page.waitForResponse(
    (response) =>
      new URL(response.url()).pathname === "/lazy" &&
      response.request().headers()["next-router-prefetch"] === "1",
  );
  await page.goto(url);
  await page.waitForFunction(
    (color) => getComputedStyle(document.querySelector("#server")).color === color,
    color,
    { timeout: 20000 },
  );
  assert.equal(
    await page.locator("#client").evaluate((el) => getComputedStyle(el).color),
    "rgb(70, 80, 90)",
  );
  await page.locator("#client").click();
  await page.waitForFunction(() => document.querySelector("#client")?.textContent === "1");
  await prefetch;
  await page.locator("#link").click();
  await page.locator("#lazy").waitFor();
  assert.equal(
    await page.locator("#lazy").evaluate((el) => getComputedStyle(el).color),
    "rgb(90, 80, 70)",
  );
  await page.locator("#back").click();
  await page.locator("#server").waitFor();
  return { color, lazy: "rgb(90, 80, 70)", prefetched: true };
}
async function stop() {
  if (proc?.child.exitCode === null) {
    proc.child.kill();
    await Promise.race([proc.done, delay(5000)]);
  }
  proc = undefined;
}
try {
  proc = start(["dev", `--${mode}`, "-p", String(port)]);
  await ready();
  browser = await chromium.launch({ channel: "chrome", headless: true });
  const page = await browser.newPage();
  page.on("pageerror", (error) => browserErrors.push(String(error)));
  page.on("requestfailed", (request) =>
    browserErrors.push(`${request.url()}: ${request.failure()?.errorText}`),
  );
  await page.goto(url);
  await page.waitForFunction(
    () => getComputedStyle(document.querySelector("#server")).color === "rgb(11, 22, 33)",
    null,
    { timeout: 20000 },
  );
  await page.locator("#client").click();
  await page.waitForFunction(() => document.querySelector("#client")?.textContent === "1");
  results.checks.push({ name: `next-${mode}-rsc-client-hydration`, passed: true });
  await write("root.css", theme("rgb(44,55,66)"));
  await page.waitForFunction(
    () => getComputedStyle(document.querySelector("#server")).color === "rgb(44, 55, 66)",
    null,
    { timeout: 30000 },
  );
  results.checks.push({ name: `next-${mode}-css-edit`, passed: true });
  assert.equal(await page.locator("#client").textContent(), "1");
  const pageSource = await fs.readFile(path.join(dir, "app/page.jsx"), "utf8");
  await write(
    "app/page.jsx",
    pageSource
      .replace("import {style} from 'stillwind/react';", "")
      .replace("{...style('text-brand')}", ""),
  );
  await page.waitForFunction(() => !document.querySelector("#server")?.className, null, {
    timeout: 30000,
  });
  assert.equal(
    await page.locator("#server").evaluate((el) => getComputedStyle(el).color),
    "rgb(0, 0, 0)",
  );
  await write("app/page.jsx", pageSource);
  await page.waitForFunction(
    () => getComputedStyle(document.querySelector("#server")).color === "rgb(44, 55, 66)",
    null,
    { timeout: 30000 },
  );
  results.checks.push({ name: `next-${mode}-macro-removal-restoration`, passed: true });

  await page.locator("#link").click();
  await page.locator("#lazy").waitFor();
  const color = await page.locator("#lazy").evaluate((el) => getComputedStyle(el).color);
  assert.equal(color, "rgb(90, 80, 70)");
  results.checks.push({ name: `next-${mode}-client-navigation`, passed: true, color });
  await stop();
  for (const temperature of ["cold", "warm"]) {
    proc = start(["build", `--${mode}`]);
    assert.equal(await proc.done, 0, proc.logs());
    const cssSourceMap = await verifyMacroSourceMap(
      path.join(dir, ".next/static"),
      "app/client.jsx",
      await fs.readFile(path.join(dir, "app/client.jsx"), "utf8"),
      "style('client')",
      true,
    );
    const javascriptSourceMap = await verifyMacroSourceMap(
      path.join(dir, ".next/static"),
      "app/client.jsx",
      await fs.readFile(path.join(dir, "app/client.jsx"), "utf8"),
      "style('client')",
    );
    proc = start(["start", "-p", String(port)]);
    await ready();
    const evidence = await verifyPage(page, "rgb(44, 55, 66)");
    results.checks.push({
      name: `next-${mode}-${temperature}-production`,
      passed: true,
      evidence,
      javascriptSourceMap,
      cssSourceMap,
    });
    await stop();
  }
} catch (error) {
  results.checks.push({
    name: `next-${mode}-failure`,
    passed: false,
    error: String(error.stack),
    browserErrors,
  });
  throw error;
} finally {
  await browser?.close();
  await stop();
  await fs.writeFile(
    path.join(root, `results/adapters-${mode}.json`),
    JSON.stringify(results, null, 2),
  );
}
console.log(JSON.stringify(results, null, 2));
