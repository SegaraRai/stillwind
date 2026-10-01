import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { chromium } from "playwright";

import { verifyMacroSourceMap } from "./verify-source-map.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const repo = path.resolve(root, "../..");
const bundler = process.argv[2] ?? "vite";
assert.ok(["vite", "rsbuild"].includes(bundler));
const dir = path.join(root, "output", `tanstack-${bundler}-${Date.now()}`);
const port = bundler === "vite" ? 43178 : 43179;
const url = `http://localhost:${port}`;
const { version } = JSON.parse(
  await fs.readFile(new URL(import.meta.resolve("@tanstack/react-start/package.json")), "utf8"),
);
const results = { framework: "TanStack Start", version, bundler, checks: [] };
const write = async (name, text) => {
  const file = path.join(dir, name);
  await fs.mkdir(path.dirname(file), { recursive: true });
  await fs.writeFile(file, text);
};
const theme = (color) =>
  `@import 'stillwind'; @utility brand {color:${color}} @utility active {color:rgb(70,80,90)} @utility lazy {color:rgb(90,80,70)} @utility server {color:rgb(20,40,60)}`;
const home = `import {useEffect,useState} from 'react';
import {createFileRoute,Link} from '@tanstack/react-router';
import {style} from 'stillwind/react';
import {createServerFn} from '@tanstack/react-start';
import {serverProps} from '../server-style';
import '../server-style.ts?stillwind.css';
const serverValue=createServerFn({method:'GET'}).handler(()=>({props:{className:serverProps.className},message:'SERVER_FUNCTION_SENTINEL'}));
export const Route=createFileRoute('/')({loader:()=>serverValue(),component:Home});
function Home(){const data=Route.useLoaderData();const [n,setN]=useState(0);useEffect(()=>{document.documentElement.dataset.hydrated='yes'},[]);return <><p id="static" {...style('brand')}>SSR</p><p id="server" {...data.props}>Server</p><button id="client" {...style(n>0?'active':'brand')} onClick={()=>setN(n+1)}>{n}</button><button id="rpc" onClick={async()=>{const data=await serverValue();document.querySelector('#server').dataset.rpc=data.message}}>RPC</button><Link id="link" to="/details" preload={false}>Lazy</Link></>}
`;
await write(
  "package.json",
  JSON.stringify({ private: true, type: "module", packageManager: "pnpm@12.3.4" }),
);
await write(
  "tsconfig.json",
  JSON.stringify({
    compilerOptions: {
      jsx: "react-jsx",
      module: "ESNext",
      moduleResolution: "Bundler",
      target: "ES2022",
      skipLibCheck: true,
      strictNullChecks: true,
    },
  }),
);
await write("root.css", theme("rgb(11,22,33)"));
const local = (name) => JSON.stringify(pathToFileURL(path.join(repo, name)).href);
await write(
  "vite.config.ts",
  `import {defineConfig} from 'vite-plus';import {tanstackStart} from '@tanstack/react-start/plugin/vite';import react from '@vitejs/plugin-react';import {nitro} from 'nitro/vite';import {stillwind} from ${local("packages/integration/dist/vite/index.js")};export default defineConfig({plugins:[tanstackStart(),stillwind({css:'root.css',types:false,emission:'split'}),nitro(),react()],nitro:{preset:'node_server'},server:{port:${port},strictPort:true,fs:{allow:[${JSON.stringify(repo)}]}},build:{sourcemap:true}});`,
);
await write(
  "rsbuild.config.mjs",
  `import {pluginReact} from '@rsbuild/plugin-react';import {tanstackStart} from '@tanstack/react-start/plugin/rsbuild';import * as bundler from ${local("packages/integration/dist/rsbuild/index.js")};export default {root:${JSON.stringify(dir)},plugins:[bundler.pluginStillwind({css:${JSON.stringify(path.join(dir, "root.css"))},root:${JSON.stringify(dir)}}),pluginReact(),tanstackStart()],server:{port:${port},strictPort:true},output:{sourceMap:{js:'source-map',css:true}},tools:{rspack:{cache:{type:'persistent',storage:{type:'filesystem',directory:${JSON.stringify(path.join(dir, "cache"))}}}}}};`,
);
await write(
  "src/router.tsx",
  "import {createRouter} from '@tanstack/react-router';import {routeTree} from './routeTree.gen';export function getRouter(){return createRouter({routeTree,defaultPreload:false})}",
);
await write(
  "src/routes/__root.tsx",
  "import {createRootRoute,Outlet,HeadContent,Scripts} from '@tanstack/react-router';export const Route=createRootRoute({component:()=> <html><head><link rel='icon' href='data:,'/><HeadContent/>{import.meta.env.DEV && <link rel='stylesheet' href='/__stillwind/style.css?environment=ssr&module=src/routes/index.tsx&module=src/server-style.ts'/>}</head><body><Outlet/><Scripts/></body></html>});",
);
await write(
  "src/server-style.ts",
  "import {style} from 'stillwind/react';export const serverProps=style('server');",
);
await write("src/routes/index.tsx", home);
await write(
  "src/routes/details.tsx",
  "import {createFileRoute,Link} from '@tanstack/react-router';import {style} from 'stillwind/react';export const Route=createFileRoute('/details')({component:()=> <><p id='lazy' {...style('lazy')}>Lazy</p><Link id='back' to='/'>Back</Link></>});",
);
let proc, browser;
function start(mode) {
  const child = spawn(
    process.execPath,
    [path.join(root, "scripts/tanstack-runner.mjs"), bundler, mode, dir],
    {
      cwd: dir,
      windowsHide: true,
      env: {
        ...process.env,
        NODE_ENV: mode === "dev" ? "development" : "production",
        PORT: String(port),
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
      document.querySelector("#client").textContent === "1" &&
      getComputedStyle(document.querySelector("#client")).color === "rgb(70, 80, 90)",
  );
  await page.locator("#rpc").click();
  await page.waitForFunction(
    () => document.querySelector("#server").dataset.rpc === "SERVER_FUNCTION_SENTINEL",
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
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await verify(page, "rgb(11, 22, 33)");
  results.checks.push({ name: "dev-ssr-hydration-server-function-lazy-navigation", passed: true });
  await write("root.css", theme("rgb(44,55,66)"));
  await page.waitForFunction(
    () => getComputedStyle(document.querySelector("#static")).color === "rgb(44, 55, 66)",
  );
  results.checks.push({ name: "dev-css-edit", passed: true });
  await write("src/routes/index.tsx", home.replace("{...style('brand')}", ""));
  await page.waitForFunction(() => !document.querySelector("#static").className);
  await write(
    "src/server-style.ts",
    "import {style} from 'stillwind/react';export const serverProps=style('server');",
  );
  await write("src/routes/index.tsx", home);
  await page.waitForFunction(
    () => getComputedStyle(document.querySelector("#static")).color === "rgb(44, 55, 66)",
  );
  results.checks.push({ name: "dev-macro-removal-restoration", passed: true });
  await write(
    "src/server-style.ts",
    "import {style} from 'stillwind/react';export const serverProps=style('[color:rgb(30,50,70)]');",
  );
  await page.reload();
  await page.waitForFunction(
    () => getComputedStyle(document.querySelector("#server")).color === "rgb(30, 50, 70)",
  );
  results.checks.push({ name: "dev-server-css-projection-edit", passed: true });
  await page.waitForFunction(() => document.documentElement.dataset.hydrated === "yes");
  await page.goto("about:blank");
  await stop();
  await write(
    "src/server-style.ts",
    "import {style} from 'stillwind/react';export const serverProps=style('server');",
  );
  for (const temperature of ["cold", "warm"]) {
    proc = start("build");
    assert.equal(await proc.done, 0, proc.logs());
    const clientDir = path.join(dir, bundler === "vite" ? ".output/public" : "dist/client");
    const cssSourceMap =
      bundler === "rsbuild"
        ? await verifyMacroSourceMap(
            clientDir,
            "src/routes/index.tsx",
            home,
            "style('brand')",
            true,
          )
        : undefined;
    const javascriptSourceMap = await verifyMacroSourceMap(
      clientDir,
      "src/routes/index.tsx",
      home,
      "style('brand')",
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
