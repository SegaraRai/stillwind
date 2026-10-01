import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs/promises";
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
const bundler = process.argv[2] || "vite";
const port = bundler === "vite" ? 43178 : 43179;
const url = "http://localhost:" + port;
const results = {
  framework: "TanStack Start",
  bundler,
  checks: [] as { name: string; passed: boolean; [key: string]: unknown }[],
};
const home = await fs.readFile(path.join(dir, "src/routes/index.tsx"), "utf8");
let proc: ReturnType<typeof start> | undefined;
let browser!: Browser;
function start(mode: string) {
  const child = spawn(
    process.execPath,
    [path.join(root, "scripts/tanstack-runner.ts"), bundler, mode, dir],
    {
      cwd: dir,
      windowsHide: true,
      env: {
        ...process.env,
        NODE_ENV: mode === "dev" ? "development" : "production",
        PORT: String(port),
        STILLWIND_E2E_PORT: String(port),
        NITRO_PORT: String(port),
      },
      stdio: ["ignore", "pipe", "pipe"],
    },
  );
  let logs = "";
  for (const stream of [child.stdout, child.stderr]) {
    stream.on("data", (data) => {
      logs += data;
      process.stdout.write(data);
    });
  }
  return {
    child,
    logs: () => logs,
    done: new Promise<void>((resolve) => child.on("exit", resolve)),
  };
}
async function stop() {
  if (proc?.child.exitCode === null) {
    await stopProcess(proc!.child);
    await proc.done;
  }
  proc = undefined;
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
    await new Promise<void>((resolve) => setTimeout(resolve, 200));
  }
  throw new Error(proc!.logs());
}
async function verify(page: Page, color: string) {
  const context = await browser.newContext({ javaScriptEnabled: false });
  try {
    const initial = await context.newPage();
    await initial.goto(url);
    assert.equal(
      await initial.locator("#static").evaluate((el) => getComputedStyle(el).color),
      color,
    );
    assert.equal(
      await initial.locator("#server").evaluate((el) => getComputedStyle(el).color),
      "rgb(20, 40, 60)",
    );
  } finally {
    await context.close();
  }
  await page.goto(url);
  await page.waitForFunction(() => document.documentElement.dataset.hydrated === "yes");
  assert.equal(await page.locator("#static").evaluate((el) => getComputedStyle(el).color), color);
  await page.locator("#client").click();
  await page.waitForFunction(
    () =>
      document.querySelector<HTMLElement>("#client")!.textContent === "1" &&
      getComputedStyle(document.querySelector<HTMLElement>("#client")!).color === "rgb(70, 80, 90)",
  );
  await page.locator("#rpc").click();
  await page.waitForFunction(
    () =>
      document.querySelector<HTMLElement>("#server")!.dataset.rpc === "SERVER_FUNCTION_SENTINEL",
  );
  const initialCss = await page.evaluate(() =>
    [...document.styleSheets]
      .flatMap((sheet) => [...sheet.cssRules].map((rule) => rule.cssText))
      .join("\n"),
  );
  await page.evaluate(() => (window.__navigationSentinel = 42));
  await page.locator("#link").click();
  await page.locator("#lazy").waitFor();
  await page.waitForFunction(
    () =>
      getComputedStyle(document.querySelector<HTMLElement>("#lazy")!).color === "rgb(90, 80, 70)",
  );
  assert.equal(
    initialCss.includes("." + (await page.locator("#lazy").getAttribute("class"))),
    bundler === "rsbuild",
  );
  assert.equal(await page.evaluate(() => window.__navigationSentinel), 42);
  await page.locator("#back").click();
  await page.locator("#static").waitFor();
}
try {
  proc = start("dev");
  await ready();
  browser = await chromium.launch({ channel: "chrome", headless: true });
  const page = await browser.newPage();
  page.setDefaultTimeout(20000);
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await verify(page, "rgb(11, 22, 33)");
  results.checks.push({
    name: "dev-ssr-hydration-server-function-lazy-navigation",
    passed: true,
  });
  await write("root.css", theme("rgb(44,55,66)"));
  await page.waitForFunction(
    () =>
      getComputedStyle(document.querySelector<HTMLElement>("#static")!).color === "rgb(44, 55, 66)",
  );
  results.checks.push({ name: "dev-css-edit", passed: true });
  await write("src/routes/index.tsx", home.replace('{...style("brand")}', ""));
  await page.waitForFunction(() => !document.querySelector<HTMLElement>("#static")!.className);
  await write(
    "src/server-style.ts",
    'import { style } from "stillwind/react";export const serverProps=style("server");',
  );
  await write("src/routes/index.tsx", home);
  await page.waitForFunction(
    () =>
      getComputedStyle(document.querySelector<HTMLElement>("#static")!).color === "rgb(44, 55, 66)",
  );
  results.checks.push({ name: "dev-macro-removal-restoration", passed: true });
  await write(
    "src/server-style.ts",
    'import { style } from "stillwind/react";export const serverProps=style("[color:rgb(30,50,70)]");',
  );
  await page.reload();
  await page.waitForFunction(
    () =>
      getComputedStyle(document.querySelector<HTMLElement>("#server")!).color === "rgb(30, 50, 70)",
  );
  results.checks.push({ name: "dev-server-css-projection-edit", passed: true });
  await page.waitForFunction(() => document.documentElement.dataset.hydrated === "yes");
  await page.goto("about:blank");
  await stop();
  await write(
    "src/server-style.ts",
    'import { style } from "stillwind/react";export const serverProps=style("server");',
  );
  for (const temperature of ["cold", "warm"]) {
    proc = start("build");
    assert.equal(await proc.done, 0, proc!.logs());
    const clientDir = path.join(dir, bundler === "vite" ? ".output/public" : "dist/client");
    const cssSourceMap =
      bundler === "rsbuild"
        ? await verifyMacroSourceMap(
            clientDir,
            "src/routes/index.tsx",
            home,
            'style("brand")',
            true,
          )
        : undefined;
    const javascriptSourceMap = await verifyMacroSourceMap(
      clientDir,
      "src/routes/index.tsx",
      home,
      'style("brand")',
    );
    for (const file of await fs.readdir(clientDir, { recursive: true })) {
      if (/\.[cm]?js$/.test(file)) {
        assert.ok(
          !(await fs.readFile(path.join(clientDir, file), "utf8")).includes(
            "SERVER_FUNCTION_SENTINEL",
          ),
          `server function leaked into ${file}`,
        );
      }
    }
    proc = start("start");
    await ready();
    await verify(page, "rgb(44, 55, 66)");
    results.checks.push({
      name: `${temperature}-ssr-hydration-server-function-lazy-navigation`,
      passed: true,
      javascriptSourceMap,
      cssSourceMap,
    });
    await page.goto("about:blank");
    await stop();
  }
  assert.deepEqual(errors, []);
  await fs.writeFile(
    path.join(root, `results/tanstack-${bundler}-real.json`),
    JSON.stringify(results, null, 2) + "\n",
  );
  console.log(JSON.stringify(results, null, 2));
} finally {
  await browser?.close();
  await stop();
}

declare global {
  interface Window {
    __navigationSentinel: number | undefined;
  }
}
