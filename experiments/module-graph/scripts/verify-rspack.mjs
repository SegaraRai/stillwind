import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import { createServer } from "node:http";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { chromium } from "playwright";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const repo = path.resolve(root, "../..");
const mode = process.argv[2] ?? "rspack";
const dir = path.join(root, "output", `${mode}-real-${Date.now()}`);
await fs.mkdir(dir, { recursive: true });
const put = (name, text) => fs.writeFile(path.join(dir, name), text);
const theme = (color) =>
  `@import ${JSON.stringify(path.relative(dir, path.join(repo, "packages/core/index.css")).replaceAll("\\", "/"))}; @theme {--color-brand:${color};} @utility lazy {color:rgb(70,80,90)}`;
await put("root.css", theme("rgb(11,22,33)"));
await put(
  "main.js",
  `import {props} from './static.js'; document.body.innerHTML='<p id="main">Main</p><button id="load">Load</button><p id="lazy">Lazy</p>'; document.querySelector('#main').className=props.className??''; document.querySelector('#load').onclick=()=>import('./lazy.js').then(m=>m.show());`,
);
await put(
  "static.js",
  `import {style} from 'stillwind/react';\nexport const props=style('text-brand bg-[url(./icon.svg)]');`,
);
await put("icon.svg", '<svg xmlns="http://www.w3.org/2000/svg"><text>asset</text></svg>');
await put(
  "lazy.js",
  `import {style} from 'stillwind/react';\nexport function show(){document.querySelector('#lazy').className=style('lazy').className;}`,
);
const results = { checks: [] };
async function build() {
  const child = spawn(
    process.execPath,
    [
      path.join(
        root,
        mode === "rsbuild"
          ? "scripts/rsbuild-adapter-build.mjs"
          : "scripts/rspack-adapter-build.cjs",
      ),
      dir,
    ],
    {
      windowsHide: true,
      env: { ...process.env, NODE_ENV: "production" },
      stdio: ["ignore", "pipe", "pipe"],
    },
  );
  let logs = "";
  child.stdout.on("data", (data) => {
    logs += data;
  });
  child.stderr.on("data", (data) => {
    logs += data;
  });
  assert.equal(await new Promise((done) => child.on("exit", done)), 0, logs);
  const stats = JSON.parse(await fs.readFile(path.join(dir, "stats.json"), "utf8"));
  const initial = stats.chunks.filter((chunk) => chunk.initial).flatMap((chunk) => chunk.files);
  await fs.writeFile(
    path.join(dir, "dist/index.html"),
    initial
      .filter((file) => file.endsWith(".css"))
      .map((file) => `<link rel="stylesheet" href="/${file}">`)
      .join("") +
      initial
        .filter((file) => file.endsWith(".js"))
        .map((file) => `<script defer src="/${file}"></script>`)
        .join(""),
  );
  return stats;
}
const server = createServer(async (req, res) => {
  try {
    const name = new URL(req.url, "http://localhost").pathname;
    const file = path.join(dir, "dist", name === "/" ? "index.html" : name);
    const body = await fs.readFile(file);
    res.setHeader("cache-control", "no-store");
    res.setHeader(
      "content-type",
      file.endsWith(".css")
        ? "text/css"
        : file.endsWith(".js")
          ? "text/javascript"
          : file.endsWith(".svg")
            ? "image/svg+xml"
            : "text/html",
    );
    res.end(body);
  } catch {
    res.statusCode = 404;
    res.end();
  }
});
let browser;
try {
  await new Promise((done) => server.listen(0, "127.0.0.1", done));
  const url = `http://127.0.0.1:${server.address().port}`;
  browser = await chromium.launch({ channel: "chrome", headless: true });
  const page = await browser.newPage();
  for (const stage of ["cold", "warm", "css-edit", "unreachable", "macro-removal"]) {
    if (stage === "css-edit") await put("root.css", theme("rgb(44,55,66)"));
    if (stage === "unreachable")
      await put(
        "main.js",
        `import {props} from './static.js'; document.body.innerHTML='<p id="main">Main</p>'; document.querySelector('#main').className=props.className??'';`,
      );
    if (stage === "macro-removal") await put("static.js", "export const props={};");
    const stats = await build();
    await page.goto(url);
    const expected =
      stage === "macro-removal"
        ? "rgb(0, 0, 0)"
        : ["cold", "warm"].includes(stage)
          ? "rgb(11, 22, 33)"
          : "rgb(44, 55, 66)";
    await page.waitForFunction(
      (expected) => getComputedStyle(document.querySelector("#main")).color === expected,
      expected,
    );
    if (stage === "macro-removal") {
      assert.equal(
        stats.chunks.flatMap((chunk) => chunk.files).filter((file) => file.endsWith(".css")).length,
        0,
      );
      results.checks.push({ name: `${mode}-macro-removal`, passed: true });
      continue;
    }
    const bg = await page.locator("#main").evaluate((el) => getComputedStyle(el).backgroundImage);
    assert.match(bg, /\.svg/);
    assert.equal((await fetch(bg.match(/url\("([^"]+)"\)/)[1])).status, 200);
    if (stage === "unreachable") {
      assert.equal(
        stats.chunks.flatMap((chunk) => chunk.files).filter((file) => file.endsWith(".css")).length,
        1,
      );
      assert(!stats.modules.some((module) => module.name === "./lazy.js"));
      results.checks.push({ name: `${mode}-unreachable-producer`, passed: true });
      continue;
    }
    await page.locator("#load").click();
    await page.waitForFunction(
      () => getComputedStyle(document.querySelector("#lazy")).color === "rgb(70, 80, 90)",
    );
    const producers = stats.modules.filter((module) => /\.\/(static|lazy)\.js$/.test(module.name));
    assert.equal(producers.length, 2);
    if (stage === "warm")
      assert(
        producers.every((module) => !module.built),
        JSON.stringify(producers),
      );
    results.checks.push({
      name: `${mode}-${stage}-browser-css`,
      passed: true,
      producers: producers.map((module) => ({ name: module.name, built: module.built })),
      cssChunks: stats.chunks
        .flatMap((chunk) => chunk.files)
        .filter((file) => file.endsWith(".css")),
    });
  }
} catch (error) {
  results.checks.push({ name: `${mode}-failure`, passed: false, error: String(error.stack) });
  throw error;
} finally {
  await browser?.close();
  await new Promise((done) => server.close(done));
  await fs.writeFile(
    path.join(root, `results/${mode}-real.json`),
    JSON.stringify(results, null, 2),
  );
}
console.log(JSON.stringify(results, null, 2));
