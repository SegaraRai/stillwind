import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { stopProcess } from "@stillwind/e2e-harness/process";
import { verifyMacroSourceMap } from "@stillwind/e2e-harness/source-map";
import { chromium, type Browser, type Page } from "playwright";

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
const initialTheme = await fs.readFile(path.join(dir, "root.css"), "utf8");
const theme = (color: string) => initialTheme.replace(/rgb\(11,\s*22,\s*33\)/, color);
const require = createRequire(import.meta.url);
const mode = process.argv[2] || "turbopack";
const standalone = process.argv[3] === "standalone";
const label = standalone ? "turbopack-standalone" : mode;
const port = standalone ? 43181 : mode === "webpack" ? 43175 : 43174;
const url = "http://localhost:" + port;
const results = {
  mode,
  standalone,
  next: require("next/package.json").version,
  checks: [] as { name: string; passed: boolean; [key: string]: unknown }[],
};
const browserErrors: string[] = [];
const delay = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));
let browser!: Browser;
let proc: ReturnType<typeof start> | undefined;
function start(args: string[]) {
  const child = spawn(process.execPath, [require.resolve("next/dist/bin/next"), ...args], {
    cwd: dir,
    windowsHide: true,
    env: {
      ...process.env,
      NODE_ENV: args[0] === "dev" ? "development" : "production",
      NEXT_TELEMETRY_DISABLED: "1",
      STILLWIND_E2E_STANDALONE: standalone ? "1" : "0",
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
    if (proc!.child.exitCode !== null) {
      throw new Error(proc!.logs());
    }
    try {
      if ((await fetch(url, { signal: AbortSignal.timeout(2000) })).ok) {
        return;
      }
    } catch {}
    await delay(300);
  }
  throw new Error(proc!.logs());
}
async function verifyPage(page: Page, color: string) {
  const prefetch = page.waitForResponse(
    (response) =>
      new URL(response.url()).pathname === "/lazy" &&
      response.request().headers()["next-router-prefetch"] === "1",
  );
  await page.goto(url);
  await page.waitForFunction(
    (color: string) =>
      getComputedStyle(document.querySelector<HTMLElement>("#server")!).color === color,
    color,
    { timeout: 20000 },
  );
  assert.equal(
    await page.locator("#client").evaluate((el) => getComputedStyle(el).color),
    "rgb(70, 80, 90)",
  );
  await page.locator("#client").click();
  await page.waitForFunction(
    () => document.querySelector<HTMLElement>("#client")?.textContent === "1",
  );
  assert.equal(
    await page.locator("#composed").evaluate((el) => getComputedStyle(el).marginTop),
    "20px",
  );
  assert.equal(
    await page.locator("#composed").evaluate((el) => getComputedStyle(el).paddingTop),
    "7px",
  );
  assert.equal(
    await page.locator("#explicit").evaluate((el) => getComputedStyle(el).paddingTop),
    "19px",
  );
  assert.match(
    await page.locator("#explicit").evaluate((el) => getComputedStyle(el).backgroundImage),
    /\/_next\/static\/media\//,
  );
  const flight = await prefetch;
  assert.equal(flight.status(), 200);
  assert.match(flight.headers()["content-type"], /text\/x-component/);
  assert.doesNotMatch(await flight.text(), /STILLWIND_SERVER_IMPLEMENTATION_SENTINEL/);
  await page.locator("#link").click();
  await page.locator("#lazy").waitFor();
  assert.equal(
    await page.locator("#lazy").evaluate((el) => getComputedStyle(el).color),
    "rgb(90, 80, 70)",
  );
  await page.locator("#back").click();
  await page.locator("#server").waitFor();
  return { color, lazy: "rgb(90, 80, 70)", prefetched: true, flight: true };
}
async function stop() {
  if (proc?.child.exitCode === null) {
    await stopProcess(proc!.child);
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
    () =>
      getComputedStyle(document.querySelector<HTMLElement>("#server")!).color === "rgb(11, 22, 33)",
    null,
    { timeout: 20000 },
  );
  await page.locator("#client").click();
  await page.waitForFunction(
    () => document.querySelector<HTMLElement>("#client")?.textContent === "1",
  );
  results.checks.push({ name: `next-${mode}-rsc-client-hydration`, passed: true });
  await write("root.css", theme("rgb(44,55,66)"));
  await page.waitForFunction(
    () =>
      getComputedStyle(document.querySelector<HTMLElement>("#server")!).color === "rgb(44, 55, 66)",
    null,
    { timeout: 30000 },
  );
  results.checks.push({ name: `next-${mode}-css-edit`, passed: true });
  assert.equal(await page.locator("#client").textContent(), "1");
  const pageSource = await fs.readFile(path.join(dir, "app/page.jsx"), "utf8");
  await write(
    "app/page.jsx",
    pageSource
      .replace('import { style } from "stillwind/react";', "")
      .replace('{...style("text-brand")}', ""),
  );
  await page.waitForFunction(
    () => !document.querySelector<HTMLElement>("#server")?.className,
    null,
    {
      timeout: 30000,
    },
  );
  assert.equal(
    await page.locator("#server").evaluate((el) => getComputedStyle(el).color),
    "rgb(0, 0, 0)",
  );
  await write("app/page.jsx", pageSource);
  await page.waitForFunction(
    () =>
      getComputedStyle(document.querySelector<HTMLElement>("#server")!).color === "rgb(44, 55, 66)",
    null,
    { timeout: 30000 },
  );
  results.checks.push({ name: `next-${mode}-macro-removal-restoration`, passed: true });

  await page.locator("#link").click();
  await page.locator("#lazy").waitFor();
  const color = await page.locator("#lazy").evaluate((el) => getComputedStyle(el).color);
  assert.equal(color, "rgb(90, 80, 70)");
  results.checks.push({ name: `next-${mode}-client-navigation`, passed: true, color });
  await page.goto(url);
  await page.waitForFunction(
    () => getComputedStyle(document.querySelector<HTMLElement>("#composed")!).marginTop === "20px",
  );
  await write(
    "shared/derived.ts",
    "import { style } from \"stillwind/react\";import {base} from './base';export const derived=style(base,'m-8');",
  );
  await page.waitForFunction(
    () => getComputedStyle(document.querySelector<HTMLElement>("#composed")!).marginTop === "32px",
  );
  await write(
    "shared/derived.ts",
    "import { style } from \"stillwind/react\";import {base} from './base';export const derived=style(base,'override');",
  );
  await page.waitForFunction(
    () => getComputedStyle(document.querySelector<HTMLElement>("#composed")!).marginTop === "20px",
  );
  results.checks.push({ name: `next-${mode}-cross-module-composition-edit`, passed: true });
  if (mode === "turbopack" && !standalone) {
    const cssText = () =>
      page.evaluate(() =>
        [...document.styleSheets]
          .flatMap((sheet) => [...sheet.cssRules].map((rule) => rule.cssText))
          .join("\n"),
      );
    await write(
      "app/added/page.jsx",
      'import { style } from "stillwind/react";export default function Added(){return <div {...style("m-[137px]")}>added</div>}',
    );
    for (let attempt = 0; attempt < 100 && !(await cssText()).includes("137px"); attempt++) {
      await delay(200);
    }
    assert.match(await cssText(), /137px/);
    await fs.unlink(path.join(dir, "app/added/page.jsx"));
    for (let attempt = 0; attempt < 100 && (await cssText()).includes("137px"); attempt++) {
      await delay(200);
    }
    assert.doesNotMatch(await cssText(), /137px/);
    results.checks.push({ name: "next-turbopack-route-addition-deletion", passed: true });
  }
  await stop();
  for (const temperature of ["cold", "warm"]) {
    proc = start(["build", `--${mode}`]);
    assert.equal(await proc.done, 0, proc!.logs());
    let serverContainsSentinel = false;
    for (const file of await fs.readdir(path.join(dir, ".next/server"), { recursive: true })) {
      if (
        file.endsWith(".js") &&
        (await fs.readFile(path.join(dir, ".next/server", file), "utf8")).includes(
          "STILLWIND_SERVER_IMPLEMENTATION_SENTINEL",
        )
      ) {
        serverContainsSentinel = true;
      }
    }
    assert.equal(
      serverContainsSentinel,
      true,
      "The server implementation must remain in the server build",
    );
    for (const file of await fs.readdir(path.join(dir, ".next/static"), { recursive: true })) {
      if (/\.(?:js|map|css)$/.test(file)) {
        assert.doesNotMatch(
          await fs.readFile(path.join(dir, ".next/static", file), "utf8"),
          /STILLWIND_SERVER_IMPLEMENTATION_SENTINEL/,
          file,
        );
      }
    }
    results.checks.push({
      name: `next-${mode}-${temperature}-server-only-isolation`,
      passed: true,
    });
    const cssSourceMap = await verifyMacroSourceMap(
      path.join(dir, ".next/static"),
      "app/client.jsx",
      await fs.readFile(path.join(dir, "app/client.jsx"), "utf8"),
      'style("client")',
      true,
    );
    const javascriptSourceMap = await verifyMacroSourceMap(
      path.join(dir, ".next/static"),
      "app/client.jsx",
      await fs.readFile(path.join(dir, "app/client.jsx"), "utf8"),
      'style("client")',
      false,
      undefined,
      mode === "turbopack" ? "call-span" : "start",
    );
    proc = start(["start", "-p", String(port)]);
    await ready();
    const ssr = await browser.newPage({ javaScriptEnabled: false });
    try {
      await ssr.goto(url);
      for (const [selector, property, expected] of [
        ["#server", "color", "rgb(44, 55, 66)"],
        ["#client", "color", "rgb(70, 80, 90)"],
        ['[data-testid="main-composed"]', "marginTop", "12px"],
        ['[data-testid="main-conditional"]', "marginTop", "40px"],
      ]) {
        assert.equal(
          await ssr
            .locator(selector)
            .evaluate(
              (element, property) =>
                getComputedStyle(element)[property as keyof CSSStyleDeclaration],
              property,
            ),
          expected,
        );
      }
      assert.equal(await ssr.locator("#client").textContent(), "0");
      results.checks.push({
        name: `next-${mode}-${temperature}-ssr-without-javascript`,
        passed: true,
      });
    } finally {
      await ssr.close();
    }
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
    error: String(error instanceof Error ? error.stack : error),
    browserErrors,
  });
  throw error;
} finally {
  await browser?.close();
  await stop();
  await fs.writeFile(
    path.join(root, `results/adapters-${label}.json`),
    JSON.stringify(results, null, 2),
  );
}
console.log(JSON.stringify(results, null, 2));
