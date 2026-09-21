import { strict as assert } from "node:assert";
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { chromium } from "playwright";
import { createServer, build } from "vite-plus";

const require = createRequire(import.meta.url);
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const runId = new Date().toISOString().replaceAll(/[:.]/g, "-");
const out = path.join(root, "output", runId);
const results = { runId, node: process.version, versions: {}, checks: [] };
for (const p of ["vite-plus", "next", "react", "playwright", "@rspack/core"]) {
  results.versions[p] = require(p + "/package.json").version;
}
const write = async (file, text) => {
  await fs.mkdir(path.dirname(file), { recursive: true });
  await fs.writeFile(file, text);
};
const delay = (ms) => new Promise((r) => setTimeout(r, ms).unref());
async function check(name, fn) {
  const start = Date.now();
  try {
    const evidence = await fn();
    results.checks.push({ name, passed: true, ms: Date.now() - start, evidence });
    console.log("PASS", name, JSON.stringify(evidence));
  } catch (e) {
    results.checks.push({ name, passed: false, error: String(e.stack) });
    console.error("FAIL", name, e);
  }
  await write(path.join(root, "results/latest.json"), JSON.stringify(results, null, 2));
}
let browser;
const processes = new Set();
function startNext(dir, args) {
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
  processes.add(child);
  let logs = "";
  child.stdout.on("data", (d) => {
    logs += d;
    process.stdout.write(d);
  });
  child.stderr.on("data", (d) => {
    logs += d;
    process.stderr.write(d);
  });
  const done = new Promise((resolve) =>
    child.on("exit", (code, signal) => {
      processes.delete(child);
      resolve({ code, signal, logs });
    }),
  );
  return { child, done, logs: () => logs };
}
async function stop(proc) {
  if (proc.child.exitCode === null) {
    proc.child.kill();
  }
  await Promise.race([proc.done, delay(5000)]);
}
async function ready(url, proc) {
  for (let i = 0; i < 40; i++) {
    if (proc.child.exitCode !== null) {
      throw new Error(proc.logs());
    }
    try {
      const r = await fetch(url, { signal: AbortSignal.timeout(1500) });
      if (r.ok) {
        return;
      }
    } catch {}
    await delay(500);
  }
  throw new Error("Server timeout\n" + proc.logs());
}
async function colors(page, url, expected) {
  await page.goto(url);
  await page.waitForFunction(
    (expected) =>
      Object.entries(expected).every(([id, color]) => {
        const el = document.getElementById(id);
        return el && getComputedStyle(el).color === color;
      }),
    expected,
    { timeout: 45000 },
  );
  return page.evaluate(
    (expected) =>
      Object.fromEntries(
        Object.keys(expected).map((id) => [
          id,
          {
            className: document.getElementById(id).className,
            color: getComputedStyle(document.getElementById(id)).color,
          },
        ]),
      ),
    expected,
  );
}
async function files(dir) {
  let names = [];
  for (const e of await fs.readdir(dir, { withFileTypes: true })) {
    names.push(
      ...(e.isDirectory() ? await files(path.join(dir, e.name)) : [path.join(dir, e.name)]),
    );
  }
  return names;
}

async function nextExperiment() {
  const dir = path.join(out, "next");
  await write(path.join(dir, "package.json"), JSON.stringify({ private: true }));
  await write(path.join(dir, "theme.txt"), "rgb(17, 34, 51)");
  await write(path.join(dir, "carrier.css"), "");
  const loader = path.join(root, "scripts/next-loader.cjs");
  const cssLoader = path.join(root, "scripts/css-loader.cjs");
  await write(
    path.join(dir, "next.config.js"),
    `module.exports = { turbopack: { root: ${JSON.stringify(root)}, rules: { '*.jsx': { loaders: [${JSON.stringify(loader)}] }, 'carrier.css': { loaders: [${JSON.stringify(cssLoader)}], as: '*.css' } } } };`,
  );
  await write(
    path.join(dir, "app/layout.jsx"),
    "export default function Layout({children}) { return <html><body>{children}</body></html> }",
  );
  await write(
    path.join(dir, "app/client.jsx"),
    `'use client';\nexport default function Client(){ return <p id="client" className={styleToken('color:rgb(70, 80, 90)')}>Client</p> }`,
  );
  await write(
    path.join(dir, "app/page.jsx"),
    `import Client from './client';\nexport default function Page(){return <><p id="server" className={styleToken('color:THEME')}>Server</p><Client/><a id="lazy-link" href="/lazy">Lazy</a></>}`,
  );
  await write(
    path.join(dir, "app/lazy/page.jsx"),
    `export default function Page(){return <p id="lazy" className={styleToken('color:rgb(101, 102, 103)')}>Lazy</p>}`,
  );
  await write(
    path.join(dir, "unused.jsx"),
    `export default styleToken('color:rgb(201, 202, 203)');`,
  );
  const expected = { server: "rgb(17, 34, 51)", client: "rgb(70, 80, 90)" };
  const page = await browser.newPage();
  let dev = startNext(dir, ["dev", "--turbopack", "-p", "43171"]);
  try {
    await check("next-dev-carrier-rsc-client", async () => {
      await ready("http://localhost:43171", dev);
      return colors(page, "http://localhost:43171", expected);
    });
    await check("next-dev-demand-graph", async () => {
      const events = await fs.readFile(path.join(dir, "loader-events.jsonl"), "utf8");
      assert(!events.includes("lazy/page.jsx"));
      assert(!events.includes("unused.jsx"));
      return { lazyUnread: true, unusedUnread: true, events };
    });
    await check("next-dev-css-dependency-hmr", async () => {
      await fs.writeFile(path.join(dir, "theme.txt"), "rgb(33, 44, 55)");
      await page.waitForFunction(
        () => getComputedStyle(document.getElementById("server")).color === "rgb(33, 44, 55)",
        null,
        { timeout: 45000 },
      );
      return { color: await page.locator("#server").evaluate((e) => getComputedStyle(e).color) };
    });
    await check("next-dev-lazy-route", () =>
      colors(page, "http://localhost:43171/lazy", { lazy: "rgb(101, 102, 103)" }),
    );
  } finally {
    await stop(dev);
    await write(path.join(root, "results/next-dev.log"), dev.logs());
  }
  for (const pass of ["cold", "warm"]) {
    const eventFile = path.join(dir, "loader-events.jsonl");
    const beforeEvents = (await fs.readFile(eventFile, "utf8")).trim().split("\n").length;
    const b = startNext(dir, ["build", "--turbopack"]);
    await check("next-build-" + pass, async () => {
      const result = await Promise.race([
        b.done,
        delay(180000).then(() => {
          throw new Error("Build timed out");
        }),
      ]);
      await write(path.join(root, `results/next-build-${pass}.log`), result.logs);
      assert.equal(result.code, 0, result.logs);
      return { exitCode: result.code };
    });
    await check("next-loader-events-" + pass, async () => {
      const afterEvents = (await fs.readFile(eventFile, "utf8")).trim().split("\n").length;
      const executed = afterEvents - beforeEvents;
      if (pass === "warm") {
        assert.equal(executed, 0);
      }
      return { executed, separateNodeProcess: true };
    });
    const server = startNext(dir, ["start", "-p", "43172"]);
    try {
      await check("next-production-" + pass, async () => {
        await ready("http://localhost:43172", server);
        const c = await colors(page, "http://localhost:43172", {
          ...expected,
          server: "rgb(33, 44, 55)",
        });
        const lazy = await colors(page, "http://localhost:43172/lazy", {
          lazy: "rgb(101, 102, 103)",
        });
        const cssFiles = (await files(path.join(dir, ".next"))).filter((f) => f.endsWith(".css"));
        const css = (await Promise.all(cssFiles.map((f) => fs.readFile(f, "utf8")))).join("\n");
        assert(!css.includes("201,202,203"));
        return { ...c, ...lazy, cssFileCount: cssFiles.length };
      });
    } finally {
      await stop(server);
    }
  }
  await page.close();
}

function vitePlugin(mode, state) {
  return {
    name: "transport-experiment",
    resolveId(id) {
      if (id === "virtual:aggregate.css" || id.startsWith("/__sw/")) {
        return id;
      }
    },
    load(id) {
      if (id === "virtual:aggregate.css") {
        state.loads.push([...state.rules.keys()]);
        return mode === "aggregate" ? [...state.rules.values()].join("\n") : "";
      }
      if (id.startsWith("/__sw/")) {
        return new URLSearchParams(id.split("?")[1]).get("css");
      }
    },
    async transform(code, id) {
      if (!id.endsWith(".js") || !code.includes("styleToken(")) {
        return;
      }
      if (id.endsWith("slow.js")) {
        await delay(350);
      }
      state.transformed.push(path.basename(id));
      let css = "";
      const replaced = code.replace(/styleToken\('([^']+)'\)/g, (_, rule) => {
        const cls =
          "sw_" +
          createHash("sha256")
            .update(path.basename(id) + rule)
            .digest("hex")
            .slice(0, 12);
        css += `.${cls}{${rule}}`;
        return JSON.stringify(cls);
      });
      state.rules.set(id, css);
      return (
        replaced +
        (mode === "carrier"
          ? `\nimport ${JSON.stringify("/__sw/" + path.basename(id) + ".css?css=" + encodeURIComponent(css))}`
          : "")
      );
    },
  };
}
async function viteExperiment() {
  const dir = path.join(out, "vite");
  await write(
    path.join(dir, "index.html"),
    '<div id="fast"></div><div id="slow"></div><button id="load">Load lazy</button><div id="lazy"></div><script type="module" src="/main.js"></script>',
  );
  await write(
    path.join(dir, "main.js"),
    `import 'virtual:aggregate.css'; import {fast} from './fast.js'; import {slow} from './slow.js'; document.querySelector('#fast').className=fast; document.querySelector('#slow').className=slow; document.querySelector('#load').onclick=async()=>{document.querySelector('#lazy').className=(await import('./lazy.js')).lazy;};`,
  );
  for (const [name, color] of [
    ["fast", "rgb(10, 20, 30)"],
    ["slow", "rgb(40, 50, 60)"],
    ["lazy", "rgb(70, 80, 90)"],
    ["unused", "rgb(100, 110, 120)"],
  ]) {
    await write(path.join(dir, name + ".js"), `export const ${name}=styleToken('color:${color}');`);
  }
  for (const mode of ["aggregate", "carrier"]) {
    const state = { rules: new Map(), loads: [], transformed: [] };
    await check("vite-build-" + mode, async () => {
      await build({
        configFile: false,
        root: dir,
        plugins: [vitePlugin(mode, state)],
        build: { outDir: path.join(dir, "dist-" + mode), minify: false, cssMinify: false },
      });
      const cssFiles = (await files(path.join(dir, "dist-" + mode))).filter((f) =>
        f.endsWith(".css"),
      );
      const css = (await Promise.all(cssFiles.map((f) => fs.readFile(f, "utf8")))).join("\n");
      const hasSlow = /40,\s*50,\s*60/.test(css);
      const hasLazy = /70,\s*80,\s*90/.test(css);
      assert(!state.transformed.includes("unused.js"));
      if (mode === "carrier") {
        assert(hasSlow);
        assert(hasLazy);
      } else {
        assert(!hasSlow, "Baseline unexpectedly includes slow CSS; examine scheduling");
      }
      return {
        aggregateLoadAt: state.loads,
        transformed: state.transformed,
        hasSlow,
        hasLazy,
        cssFileCount: cssFiles.length,
        expectedBrokenBaseline: mode === "aggregate",
      };
    });
  }
  const state = { rules: new Map(), loads: [], transformed: [] };
  const server = await createServer({
    configFile: false,
    root: dir,
    plugins: [vitePlugin("carrier", state)],
    server: { port: 43173 },
  });
  await server.listen();
  const page = await browser.newPage();
  try {
    await check("vite-dev-demand-and-lazy", async () => {
      const before = await colors(page, "http://localhost:43173", {
        fast: "rgb(10, 20, 30)",
        slow: "rgb(40, 50, 60)",
      });
      assert(!state.transformed.includes("lazy.js"));
      await page.click("#load");
      await page.waitForFunction(
        () => getComputedStyle(document.getElementById("lazy")).color === "rgb(70, 80, 90)",
      );
      return {
        before,
        transformed: [...state.transformed],
        unusedUnread: !state.transformed.includes("unused.js"),
      };
    });
    await check("vite-dev-edit-and-remove", async () => {
      await fs.writeFile(
        path.join(dir, "fast.js"),
        "export const fast=styleToken('color:rgb(120, 130, 140)');",
      );
      await page.waitForFunction(
        () => getComputedStyle(document.getElementById("fast")).color === "rgb(120, 130, 140)",
      );
      await fs.writeFile(path.join(dir, "fast.js"), "export const fast='';");
      await page.waitForFunction(
        () =>
          document.getElementById("fast").className === "" &&
          getComputedStyle(document.getElementById("fast")).color === "rgb(0, 0, 0)",
      );
      return { editedColorObserved: true, removedMacroObserved: true };
    });
  } finally {
    await page.close();
    await server.close();
  }
}
async function rspackExperiment() {
  const dir = path.join(out, "rspack");
  await write(path.join(dir, "theme.txt"), "rgb(21, 22, 23)");
  await write(path.join(dir, "carrier.css"), "");
  await write(
    path.join(dir, "main.js"),
    "document.body.className=styleToken('color:THEME'); window.lazy=()=>import('./lazy.js');",
  );
  await write(path.join(dir, "lazy.js"), "export const lazy=styleToken('color:rgb(31, 32, 33)');");
  await write(
    path.join(dir, "unused.js"),
    "export const unused=styleToken('color:rgb(201, 202, 203)');",
  );
  for (const pass of ["cold", "warm", "changed"]) {
    if (pass === "changed") {
      await fs.writeFile(path.join(dir, "theme.txt"), "rgb(41, 42, 43)");
    }
    await check("rspack-" + pass, async () => {
      const before = await fs
        .readFile(path.join(dir, "loader-events.jsonl"), "utf8")
        .catch(() => "");
      const child = spawn(process.execPath, [path.join(root, "scripts/rspack-build.cjs"), dir], {
        cwd: dir,
        windowsHide: true,
        stdio: ["ignore", "pipe", "pipe"],
      });
      processes.add(child);
      let log = "";
      child.stdout.on("data", (d) => {
        log += d;
      });
      child.stderr.on("data", (d) => {
        log += d;
      });
      const code = await new Promise((r) => child.on("exit", r));
      processes.delete(child);
      assert.equal(code, 0, log);
      const stats = JSON.parse(await fs.readFile(path.join(dir, "stats.json"), "utf8"));
      const cssAssets = [...new Set(stats.chunks.flatMap((c) => c.files))]
        .filter((name) => name.endsWith(".css"))
        .map((name) => ({ name }));
      const css = (
        await Promise.all(cssAssets.map((a) => fs.readFile(path.join(dir, "dist", a.name), "utf8")))
      ).join("\n");
      const events = (await fs.readFile(path.join(dir, "loader-events.jsonl"), "utf8"))
        .slice(before.length)
        .trim()
        .split("\n")
        .filter(Boolean)
        .map(JSON.parse);
      assert(!events.some((e) => e.owner === "unused.js"));
      assert(
        (pass === "changed"
          ? /color:\s*rgb\(41,\s*42,\s*43\)/
          : /color:\s*rgb\(21,\s*22,\s*23\)/
        ).test(css),
      );
      assert(/color:\s*rgb\(31,\s*32,\s*33\)/.test(css));
      if (pass === "warm") {
        assert.equal(events.length, 0);
      }
      if (pass === "changed") {
        assert(events.length > 0);
      }
      return {
        cssAssets: cssAssets.map((a) => a.name),
        executedLoaders: events,
        moduleCount: stats.modules.length,
        separateNodeProcess: true,
      };
    });
  }
}
async function saltExperiment() {
  const page = await browser.newPage();
  await check("salt-reversed-css-order", async () => {
    const values = [];
    for (const rules of [
      ".moduleA{color:red}.moduleB{color:blue}",
      ".moduleB{color:blue}.moduleA{color:red}",
    ]) {
      await page.setContent(
        `<style>${rules}</style><p id="a" class="moduleA">A</p><p id="b" class="moduleB">B</p><p id="both" class="moduleA moduleB">Both</p>`,
      );
      values.push(
        await page.evaluate(() =>
          Object.fromEntries(
            ["a", "b", "both"].map((id) => [
              id,
              getComputedStyle(document.getElementById(id)).color,
            ]),
          ),
        ),
      );
    }
    assert.equal(values[0].a, values[1].a);
    assert.equal(values[0].b, values[1].b);
    assert.notEqual(values[0].both, values[1].both);
    return { separateElementsStable: true, explicitCompositionOrderDependent: true, values };
  });
  await page.close();
}
try {
  browser = await chromium.launch({ channel: "chrome", headless: true });
  await saltExperiment();
  await viteExperiment();
  await rspackExperiment();
  await nextExperiment();
} catch (e) {
  results.fatal = String(e.stack);
  console.error(e);
} finally {
  for (const p of processes) {
    p.kill();
  }
  await browser?.close();
  await write(path.join(root, "results/latest.json"), JSON.stringify(results, null, 2));
}
process.exitCode = results.fatal || results.checks.some((c) => !c.passed) ? 1 : 0;
