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
const port = 43176;
const url = "http://localhost:" + port;
const results = {
  framework: "react-router",
  version: require("@react-router/dev/package.json").version,
  checks: [] as { name: string; passed: boolean; [key: string]: unknown }[],
};
const home = await fs.readFile(path.join(dir, "app/routes/home.jsx"), "utf8");
const serverSource = await fs.readFile(path.join(dir, "app/server.server.js"), "utf8");
let processHandle: ReturnType<typeof start> | undefined;
let browser!: Browser;
const pause = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));
function start(kind: string) {
  const executable = path.join(
    path.dirname(
      require.resolve(`@react-router/${kind === "start" ? "serve" : "dev"}/package.json`),
    ),
    "bin.cjs",
  );
  const args = kind === "start" ? ["build/server/index.js"] : [kind];
  const child = spawn(process.execPath, [executable, ...args], {
    cwd: dir,
    windowsHide: true,
    env: {
      ...process.env,
      NODE_OPTIONS: kind === "dev" ? "--conditions=development" : "",
      NODE_ENV: kind === "dev" ? "development" : "production",
      PORT: String(port),
      STILLWIND_E2E_PORT: String(port),
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
  if (processHandle?.child.exitCode === null) {
    await stopProcess(processHandle!.child);
    await processHandle.done;
  }
  processHandle = undefined;
}
async function ready() {
  for (let n = 0; n < 100; n++) {
    if (processHandle!.child.exitCode !== null) {
      throw new Error(processHandle!.logs());
    }
    try {
      if ((await fetch(url, { signal: AbortSignal.timeout(2000) })).ok) {
        return;
      }
    } catch {}
    await pause(200);
  }
  throw new Error(processHandle!.logs());
}
async function verify(page: Page, color: string) {
  const noJs = await browser.newContext({ javaScriptEnabled: false });
  try {
    const initial = await noJs.newPage();
    await initial.goto(url);
    await fs.writeFile(
      path.join(process.env.STILLWIND_E2E_RESULTS || path.join(root, "results"), "router-ssr.html"),
      await initial.content(),
    );
    assert.equal(
      await initial.locator("#client").evaluate((el) => getComputedStyle(el).color),
      color,
    );
    assert.equal(
      await initial.locator("#server").evaluate((el) => getComputedStyle(el).color),
      "rgb(21, 32, 43)",
    );
  } finally {
    await noJs.close();
  }
  await page.goto(url);
  await page.waitForFunction(() => document.documentElement.dataset.hydrated === "yes");
  await page.waitForFunction(
    (color: string) =>
      getComputedStyle(document.querySelector<HTMLElement>("#client")!).color === color,
    color,
  );
  await page.locator("#client").click();
  await page.waitForFunction(
    () => document.querySelector<HTMLElement>("#client")?.textContent === "1",
  );
  assert.equal(await page.evaluate(() => window.__stillwindServerMarker), undefined);
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
  const lazyClass = await page.locator("#lazy").getAttribute("class");
  assert.ok(!initialCss.includes(`.${lazyClass}`), "Lazy CSS was present before navigation");
  assert.equal(await page.evaluate(() => window.__navigationSentinel), 42);
  await page.locator("#back").click();
  await page.locator("#client").waitFor();
}
try {
  processHandle = start("dev");
  await ready();
  browser = await chromium.launch({ channel: "chrome", headless: true });
  const page = await browser.newPage();
  page.setDefaultTimeout(20000);
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await verify(page, "rgb(11, 22, 33)");
  results.checks.push({ name: "dev-ssr-hydration-navigation", passed: true });
  await write("root.css", theme("rgb(44,55,66)"));
  await page.waitForFunction(
    () =>
      getComputedStyle(document.querySelector<HTMLElement>("#client")!).color === "rgb(44, 55, 66)",
  );
  results.checks.push({ name: "dev-css-edit", passed: true });
  for (let edit = 0; edit < 3; edit++) {
    await write(
      "app/routes/home.jsx",
      home

        .replace('{...style("brand")}', ""),
    );
    await page.waitForFunction(() => !document.querySelector<HTMLElement>("#client")!.className);
    assert.equal(
      await page.locator("#client").evaluate((el) => getComputedStyle(el).color),
      "rgb(0, 0, 0)",
    );
    await write("app/routes/home.jsx", home);
    await page.waitForFunction(
      () =>
        getComputedStyle(document.querySelector<HTMLElement>("#client")!).color ===
        "rgb(44, 55, 66)",
    );
  }
  results.checks.push({ name: "dev-macro-removal-restoration", passed: true });
  await write("app/server.server.js", serverSource.replace('style("server")', 'style("lazy")'));
  await page.waitForFunction(
    () =>
      getComputedStyle(document.querySelector<HTMLElement>("#server")!).color === "rgb(90, 80, 70)",
  );
  await write("app/server.server.js", serverSource);
  await page.waitForFunction(
    () =>
      getComputedStyle(document.querySelector<HTMLElement>("#server")!).color === "rgb(21, 32, 43)",
  );
  results.checks.push({ name: "dev-server-css-projection-edit", passed: true });
  await stop();
  for (const temperature of ["cold", "warm"]) {
    processHandle = start("build");
    assert.equal(await processHandle.done, 0, processHandle!.logs());
    for (const entry of await fs.readdir(path.join(dir, "build/client"), { recursive: true })) {
      if (/\.(js|map)$/.test(entry)) {
        assert.ok(
          !(await fs.readFile(path.join(dir, "build/client", entry), "utf8")).includes(
            "SERVER_CODE_ONLY",
          ),
          "Server code leaked to client artifacts",
        );
      }
    }
    const mapped = await verifyMacroSourceMap(
      path.join(dir, "build/client/assets"),
      "app/routes/home.jsx",
      home,
      'style("brand")',
    );
    processHandle = start("start");
    await ready();
    await verify(page, "rgb(44, 55, 66)");
    results.checks.push({
      name: `${temperature}-production-ssr-hydration-navigation`,
      passed: true,
      javascriptSourceMap: { line: mapped.line, column: mapped.column },
      serverCodeExcluded: true,
      ssrWithoutJavaScript: true,
      lazyCssAbsentBeforeNavigation: true,
    });
    await stop();
  }
  assert.deepEqual(errors, []);
  await fs.writeFile(
    path.join(root, "results/react-router-real.json"),
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
    __stillwindServerMarker: unknown;
  }
}
