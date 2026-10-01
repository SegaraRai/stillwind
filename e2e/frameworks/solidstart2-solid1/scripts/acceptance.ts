import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import path from "node:path";
import { setTimeout as delay } from "node:timers/promises";
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
const port = 43177;
const url = "http://localhost:" + port;
const results = {
  framework: "SolidStart",
  checks: [] as { name: string; passed: boolean; [key: string]: unknown }[],
};
const home = await fs.readFile(path.join(dir, "src/routes/index.jsx"), "utf8");
let proc: ReturnType<typeof start> | undefined;
let browser!: Browser;
function start(mode: string) {
  const args =
    mode === "start"
      ? [path.join(dir, ".output/server/index.mjs")]
      : [path.join(root, "scripts/solidstart-runner.ts"), mode, dir];
  const child = spawn(process.execPath, args, {
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
  });
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
  const ssr = await browser.newContext({ javaScriptEnabled: false });
  try {
    const initial = await ssr.newPage();
    await initial.goto(url);
    assert.equal(
      await initial.locator("#static").evaluate((el) => getComputedStyle(el).color),
      color,
    );
    assert.equal(await initial.locator("#client").textContent(), "-1");
  } finally {
    await ssr.close();
  }
  await page.goto(url);
  await page.waitForFunction(() => document.documentElement.dataset.hydrated === "yes");
  assert.equal(await page.locator("#static").evaluate((el) => getComputedStyle(el).color), color);
  assert.equal(await page.locator("#client").evaluate((el) => getComputedStyle(el).color), color);
  await page.locator("#client").click();
  await page.waitForFunction(
    () =>
      document.querySelector<HTMLElement>("#client")!.textContent === "1" &&
      getComputedStyle(document.querySelector<HTMLElement>("#client")!).color === "rgb(70, 80, 90)",
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
  assert.ok(!initialCss.includes("." + (await page.locator("#lazy").getAttribute("class"))));
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
  results.checks.push({ name: "dev-ssr-hydration-reactivity-lazy-navigation", passed: true });
  await write("root.css", theme("rgb(44,55,66)"));
  await page.waitForFunction(
    () =>
      getComputedStyle(document.querySelector<HTMLElement>("#static")!).color === "rgb(44, 55, 66)",
  );
  results.checks.push({ name: "dev-css-edit", passed: true });
  await write("src/routes/index.jsx", home.replace('{...style("brand")}', ""));
  await page.waitForFunction(() => !document.querySelector<HTMLElement>("#static")!.className);
  // Let the file watcher finish the removal before issuing the next disk edit.
  await delay(300);
  await write("src/routes/index.jsx", home);
  await page.waitForFunction(
    () =>
      getComputedStyle(document.querySelector<HTMLElement>("#static")!).color === "rgb(44, 55, 66)",
  );
  results.checks.push({ name: "dev-macro-removal-restoration", passed: true });
  await stop();
  for (const temperature of ["cold", "warm"]) {
    proc = start("build");
    assert.equal(await proc.done, 0, proc!.logs());
    const javascriptSourceMap = await verifyMacroSourceMap(
      path.join(dir, ".output/public"),
      "src/routes/index.jsx",
      home,
      'style("brand")',
    );
    proc = start("start");
    await ready();
    await verify(page, "rgb(44, 55, 66)");
    results.checks.push({
      name: `${temperature}-ssr-hydration-reactivity-lazy-navigation`,
      passed: true,
      javascriptSourceMap,
    });
    await stop();
  }
  assert.deepEqual(errors, []);
  await fs.writeFile(
    path.join(root, "results/solidstart-real.json"),
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
