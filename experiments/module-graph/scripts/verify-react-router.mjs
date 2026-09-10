import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { TraceMap, originalPositionFor } from "@jridgewell/trace-mapping";
import { chromium } from "playwright";

const require = createRequire(import.meta.url);
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const repo = path.resolve(root, "../..");
const dir = path.join(root, "output", `react-router-${Date.now()}`);
const port = 43176;
const url = `http://localhost:${port}`;
const results = {
  framework: "react-router",
  version: require("@react-router/dev/package.json").version,
  checks: [],
};
const write = async (name, contents) => {
  const file = path.join(dir, name);
  await fs.mkdir(path.dirname(file), { recursive: true });
  await fs.writeFile(file, contents);
};
const theme = (color) =>
  `@import 'stillwind'; @utility brand { color:${color}; } @utility server {color:rgb(21,32,43)} @utility lazy {color:rgb(90,80,70)}`;
const home = `import {useEffect,useState} from 'react';
import {Link, useLoaderData} from 'react-router';
import {style} from 'stillwind/react';
import {serverProps} from '../server.server';
import '../server.server.js?stillwind.css';
export function loader(){return {props:serverProps};}
export default function Home(){const data=useLoaderData(); const [n,setN]=useState(0); useEffect(()=>{document.documentElement.dataset.hydrated='yes';},[]); return <><p id="server" {...data.props}>Server only style</p><button id="client" {...style('brand')} onClick={()=>setN(n+1)}>{n}</button><Link id="link" to="/lazy">Lazy</Link></>}
`;
await write(
  "package.json",
  JSON.stringify({
    private: true,
    type: "module",
    packageManager: "pnpm@12.3.4",
    dependencies: { isbot: "5.2.2" },
  }),
);
await write("root.css", theme("rgb(11,22,33)"));
await write(
  "vite.config.ts",
  `import {defineConfig} from 'vite-plus';
import {reactRouter} from '@react-router/dev/vite';
import {stillwind} from ${JSON.stringify(path.join(repo, "packages/vite/src/index.ts").replaceAll("\\", "/"))};
export default defineConfig({plugins:[stillwind({css:'root.css',types:false}),reactRouter()],build:{sourcemap:true},server:{port:${port},strictPort:true}});`,
);
await write("react-router.config.ts", "export default {ssr:true};");
await write(
  "app/root.jsx",
  `import {Links,Meta,Outlet,Scripts,ScrollRestoration} from 'react-router';
export default function Root(){return <html><head><link rel="icon" href="data:,"/><Meta/><Links/></head><body><Outlet/><ScrollRestoration/><Scripts/></body></html>}`,
);
await write(
  "app/routes.ts",
  "import {index,route} from '@react-router/dev/routes'; export default [index('routes/home.jsx'),route('lazy','routes/lazy.jsx')];",
);
const serverSource =
  "import {style} from 'stillwind/react'; globalThis.__stillwindServerMarker='SERVER_CODE_ONLY'; export const serverProps=style('server');";
await write("app/server.server.js", serverSource);
await write("app/routes/home.jsx", home);
await write(
  "app/routes/lazy.jsx",
  "import {Link} from 'react-router'; import {style} from 'stillwind/react'; export default function Lazy(){return <><p id='lazy' {...style('lazy')}>Lazy</p><Link id='back' to='/'>Back</Link></>}",
);
await write(
  "app/unreachable.jsx",
  "import {style} from 'stillwind/react'; export const unused=style('bg-[#123456]');",
);
let processHandle;
let browser;
const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
function start(kind) {
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
    },
    stdio: ["ignore", "pipe", "pipe"],
  });
  let logs = "";
  for (const stream of [child.stdout, child.stderr])
    stream.on("data", (data) => {
      logs += data;
      process.stdout.write(data);
    });
  return { child, logs: () => logs, done: new Promise((resolve) => child.on("exit", resolve)) };
}
async function stop() {
  if (processHandle?.child.exitCode === null) {
    processHandle.child.kill();
    await processHandle.done;
  }
  processHandle = undefined;
}
async function ready() {
  for (let n = 0; n < 100; n++) {
    if (processHandle.child.exitCode !== null) throw new Error(processHandle.logs());
    try {
      if ((await fetch(url, { signal: AbortSignal.timeout(2000) })).ok) return;
    } catch {}
    await pause(200);
  }
  throw new Error(processHandle.logs());
}
async function verify(page, color) {
  const noJs = await browser.newContext({ javaScriptEnabled: false });
  try {
    const initial = await noJs.newPage();
    await initial.goto(url);
    await fs.writeFile(path.join(root, "output", "router-ssr-debug.html"), await initial.content());
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
    (color) => getComputedStyle(document.querySelector("#client")).color === color,
    color,
  );
  await page.locator("#client").click();
  await page.waitForFunction(() => document.querySelector("#client")?.textContent === "1");
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
    () => getComputedStyle(document.querySelector("#lazy")).color === "rgb(90, 80, 70)",
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
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await verify(page, "rgb(11, 22, 33)");
  results.checks.push({ name: "dev-ssr-hydration-navigation", passed: true });
  await write("root.css", theme("rgb(44,55,66)"));
  await page.waitForFunction(
    () => getComputedStyle(document.querySelector("#client")).color === "rgb(44, 55, 66)",
  );
  results.checks.push({ name: "dev-css-edit", passed: true });
  for (let edit = 0; edit < 3; edit++) {
    await write(
      "app/routes/home.jsx",
      home.replace("import {style} from 'stillwind/react';", "").replace("{...style('brand')}", ""),
    );
    await page.waitForFunction(() => !document.querySelector("#client").className);
    assert.equal(
      await page.locator("#client").evaluate((el) => getComputedStyle(el).color),
      "rgb(0, 0, 0)",
    );
    await write("app/routes/home.jsx", home);
    await page.waitForFunction(
      () => getComputedStyle(document.querySelector("#client")).color === "rgb(44, 55, 66)",
    );
  }
  results.checks.push({ name: "dev-macro-removal-restoration", passed: true });
  await write("app/server.server.js", serverSource.replace("style('server')", "style('lazy')"));
  await page.waitForFunction(
    () => getComputedStyle(document.querySelector("#server")).color === "rgb(90, 80, 70)",
  );
  await write("app/server.server.js", serverSource);
  await page.waitForFunction(
    () => getComputedStyle(document.querySelector("#server")).color === "rgb(21, 32, 43)",
  );
  results.checks.push({ name: "dev-server-css-projection-edit", passed: true });
  await stop();
  for (const temperature of ["cold", "warm"]) {
    processHandle = start("build");
    assert.equal(await processHandle.done, 0, processHandle.logs());
    for (const entry of await fs.readdir(path.join(dir, "build/client"), { recursive: true })) {
      if (/\.(js|map)$/.test(entry))
        assert.ok(
          !(await fs.readFile(path.join(dir, "build/client", entry), "utf8")).includes(
            "SERVER_CODE_ONLY",
          ),
          "Server code leaked to client artifacts",
        );
    }
    const homeAsset = (await fs.readdir(path.join(dir, "build/client/assets"))).find((name) =>
      /^home-.*\.js$/.test(name),
    );
    const homeCode = await fs.readFile(path.join(dir, "build/client/assets", homeAsset), "utf8");
    const generated = homeCode.indexOf("_s_");
    assert.ok(generated >= 0);
    const before = homeCode.slice(0, generated);
    const mapped = originalPositionFor(
      new TraceMap(
        JSON.parse(
          await fs.readFile(path.join(dir, "build/client/assets", homeAsset + ".map"), "utf8"),
        ),
      ),
      { line: before.split("\n").length, column: before.length - before.lastIndexOf("\n") - 1 },
    );
    assert.ok(mapped.source.endsWith("app/routes/home.jsx"), JSON.stringify(mapped));
    const original = home.slice(0, home.indexOf("style('brand')"));
    assert.equal(mapped.line, original.split("\n").length);
    assert.equal(mapped.column, original.length - original.lastIndexOf("\n") - 1);
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
