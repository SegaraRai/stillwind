import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import path from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { fileURLToPath } from "node:url";

import { chromium } from "playwright";

import { verifyMacroSourceMap } from "./verify-source-map.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const repo = path.resolve(root, "../..");
const dir = path.join(root, "output", `solidstart-${Date.now()}`);
const port = 43177;
const url = `http://localhost:${port}`;
const results = { framework: "SolidStart", version: "2.0.4", checks: [] };
const write = async (name, text) => {
  const file = path.join(dir, name);
  await fs.mkdir(path.dirname(file), { recursive: true });
  await fs.writeFile(file, text);
};
const theme = (color) =>
  `@import 'stillwind'; @utility brand {color:${color}} @utility active {color:rgb(70,80,90)} @utility lazy {color:rgb(90,80,70)}`;
const home = `import {createSignal,onMount} from 'solid-js';
import {A} from '@solidjs/router';
import {style} from 'stillwind/solid';
export default function Home(){const [n,setN]=createSignal(-1);onMount(()=>document.documentElement.dataset.hydrated='yes');return <><p id="static" {...style('brand')}>SSR</p><button id="client" {...style(n()>0?'active':'brand')} onClick={()=>setN(n()+2)}>{n()}</button><A id="link" href="/lazy">Lazy</A></>}
`;
await write(
  "package.json",
  JSON.stringify({ private: true, type: "module", packageManager: "pnpm@12.3.4" }),
);
await write("root.css", theme("rgb(11,22,33)"));
await write(
  "vite.config.ts",
  `import {defineConfig} from 'vite-plus'; import {solidStart} from '@solidjs/start/config'; import {nitro} from 'nitro/vite';
import {stillwind} from ${JSON.stringify(path.join(repo, "packages/integration/dist/vite/index.js").replaceAll("\\", "/"))};
export default defineConfig({plugins:[stillwind({css:'root.css',types:false,emission:'split'}),solidStart({devOverlay:false}),nitro()],nitro:{preset:'node_server'},server:{port:${port},strictPort:true,fs:{allow:[${JSON.stringify(repo)}]}},build:{sourcemap:true}});`,
);
await write(
  "src/app.jsx",
  "import {Suspense} from 'solid-js'; import {Router} from '@solidjs/router'; import {FileRoutes} from '@solidjs/start/router'; export default function App(){return <Router root={props=><Suspense>{props.children}</Suspense>}><FileRoutes/></Router>}",
);
await write(
  "src/entry-client.jsx",
  "import {mount,StartClient} from '@solidjs/start/client'; mount(()=><StartClient/>,document.getElementById('app'));",
);
await write(
  "src/entry-server.jsx",
  "import {createHandler,StartServer} from '@solidjs/start/server'; function Document(props){return <html><head><link rel='icon' href='data:,'/>{props.assets}{import.meta.env.DEV && <link rel='stylesheet' href='/__stillwind/style.css?environment=ssr&module=src/routes/index.jsx'/>}</head><body><div id='app'>{props.children}</div>{props.scripts}</body></html>} export default createHandler(()=><StartServer document={Document}/>);",
);
await write("src/routes/index.jsx", home);
await write(
  "src/routes/lazy.jsx",
  "import {A} from '@solidjs/router'; import {style} from 'stillwind/solid'; export default function Lazy(){return <><p id='lazy' {...style('lazy')}>Lazy</p><A id='back' href='/'>Back</A></>}",
);
let proc, browser;
function start(mode) {
  const args =
    mode === "start"
      ? [path.join(dir, ".output/server/index.mjs")]
      : [path.join(root, "scripts/solidstart-runner.mjs"), mode, dir];
  const child = spawn(process.execPath, args, {
    cwd: dir,
    windowsHide: true,
    env: {
      ...process.env,
      NODE_ENV: mode === "dev" ? "development" : "production",
      PORT: String(port),
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
  return { child, logs: () => logs, done: new Promise((resolve) => child.on("exit", resolve)) };
}
async function stop() {
  if (proc?.child.exitCode === null) {
    proc.child.kill();
    await proc.done;
  }
  proc = undefined;
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
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
  throw new Error(proc.logs());
}
async function verify(page, color) {
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
      document.querySelector("#client").textContent === "1" &&
      getComputedStyle(document.querySelector("#client")).color === "rgb(70, 80, 90)",
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
    () => getComputedStyle(document.querySelector("#lazy")).color === "rgb(90, 80, 70)",
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
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await verify(page, "rgb(11, 22, 33)");
  results.checks.push({ name: "dev-ssr-hydration-reactivity-lazy-navigation", passed: true });
  await write("root.css", theme("rgb(44,55,66)"));
  await page.waitForFunction(
    () => getComputedStyle(document.querySelector("#static")).color === "rgb(44, 55, 66)",
  );
  results.checks.push({ name: "dev-css-edit", passed: true });
  await write("src/routes/index.jsx", home.replace("{...style('brand')}", ""));
  await page.waitForFunction(() => !document.querySelector("#static").className);
  // Let the file watcher finish the removal before issuing the next disk edit.
  await delay(300);
  await write("src/routes/index.jsx", home);
  await page.waitForFunction(
    () => getComputedStyle(document.querySelector("#static")).color === "rgb(44, 55, 66)",
  );
  results.checks.push({ name: "dev-macro-removal-restoration", passed: true });
  await stop();
  for (const temperature of ["cold", "warm"]) {
    proc = start("build");
    assert.equal(await proc.done, 0, proc.logs());
    const javascriptSourceMap = await verifyMacroSourceMap(
      path.join(dir, ".output/public"),
      "src/routes/index.jsx",
      home,
      "style('brand')",
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
