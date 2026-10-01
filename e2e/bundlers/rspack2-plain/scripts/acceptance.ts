import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import { createServer } from "node:http";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { TraceMap, originalPositionFor, sourceContentFor } from "@jridgewell/trace-mapping";
import { resolveSourceMapFile } from "@stillwind/e2e-harness/source-map";
import { chromium, type Browser } from "playwright";

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
const put = write;
const initialTheme = await fs.readFile(path.join(dir, "root.css"), "utf8");
const theme = (color: string) => initialTheme.replace(/rgb\(11,\s*22,\s*33\)/, color);
const mode = process.argv[2] || "rspack";
const results = { checks: [] as { name: string; passed: boolean; [key: string]: unknown }[] };
async function build() {
  const child = spawn(
    process.execPath,
    [path.join(root, "scripts/rspack-adapter-build.cjs"), dir],
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
  const stats: {
    chunks: { initial: boolean; files: string[] }[];
    modules: { name: string; built: boolean }[];
  } = JSON.parse(await fs.readFile(path.join(dir, "stats.json"), "utf8"));
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
    const name = new URL(req.url ?? "/", "http://localhost").pathname;
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
let browser!: Browser;
try {
  await new Promise<void>((done) => server.listen(0, "127.0.0.1", done));
  const url = `http://127.0.0.1:${(server.address() as import("node:net").AddressInfo).port}`;
  browser = await chromium.launch({ channel: "chrome", headless: true });
  const page = await browser.newPage();
  for (const stage of ["cold", "warm", "css-edit", "unreachable", "macro-removal"]) {
    if (stage === "css-edit") {
      await put("root.css", theme("rgb(44,55,66)"));
    }
    if (stage === "unreachable") {
      await put(
        "main.js",
        `import {props} from './static.js'; document.body.innerHTML='<p id="main">Main</p>'; document.querySelector('#main').className=props.className??'';`,
      );
    }
    if (stage === "macro-removal") {
      await put("static.js", "export const props={};");
    }
    const stats = await build();
    if (stage === "cold" || stage === "warm") {
      const cssFile = stats.chunks
        .filter((chunk) => chunk.initial)
        .flatMap((chunk) => chunk.files)
        .find((file) => file.endsWith(".css"));
      assert.ok(cssFile);
      const cssText = await fs.readFile(path.join(dir, "dist", cssFile), "utf8");
      const cssMap = new TraceMap(
        JSON.parse(await fs.readFile(path.join(dir, "dist", cssFile + ".map"), "utf8")),
      );
      const offset = [...cssText.matchAll(/\.sw_/g)].find(({ index }) => {
        const prefix = cssText.slice(0, index);
        const origin = originalPositionFor(cssMap, {
          line: prefix.split("\n").length,
          column: index - prefix.lastIndexOf("\n") - 1,
        });
        return (
          origin.source &&
          resolveSourceMapFile(origin.source, path.join(dir, "dist", cssFile + ".map"), dir) ===
            path.join(dir, "static.js")
        );
      })?.index;
      assert.ok(
        offset !== undefined,
        JSON.stringify({ sources: cssMap.sources, root: dir, cssFile }),
      );
      const prefix = cssText.slice(0, offset);
      const origin = originalPositionFor(cssMap, {
        line: prefix.split("\n").length,
        column: offset - prefix.lastIndexOf("\n") - 1,
      });
      assert(origin.source, JSON.stringify(origin));
      assert.equal(
        resolveSourceMapFile(origin.source, path.join(dir, "dist", cssFile + ".map"), dir),
        path.join(dir, "static.js"),
      );
      const source = await fs.readFile(path.join(dir, "static.js"), "utf8");
      const macroOffset = source.indexOf("style(");
      assert.notEqual(macroOffset, -1);
      const sourcePrefix = source.slice(0, macroOffset);
      assert.equal(origin.line, sourcePrefix.split("\n").length);
      assert.equal(origin.column, macroOffset - sourcePrefix.lastIndexOf("\n") - 1);
      assert.equal(
        sourceContentFor(cssMap, origin.source),
        await fs.readFile(path.join(dir, "static.js"), "utf8"),
      );
      results.checks.push({ name: `${mode}-${stage}-css-source-map`, passed: true });
    }
    await page.goto(url);
    const expected =
      stage === "macro-removal"
        ? "rgb(0, 0, 0)"
        : ["cold", "warm"].includes(stage)
          ? "rgb(11, 22, 33)"
          : "rgb(44, 55, 66)";
    await page.waitForFunction(
      (expected: string) =>
        getComputedStyle(document.querySelector<HTMLElement>("#main")!).color === expected,
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
    assert.equal((await fetch(bg.match(/url\("([^"]+)"\)/)![1])).status, 200);
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
      () =>
        getComputedStyle(document.querySelector<HTMLElement>("#lazy")!).color === "rgb(70, 80, 90)",
    );
    const producers = stats.modules.filter((module) => /\.\/(static|lazy)\.js$/.test(module.name));
    assert.equal(producers.length, 2);
    if (stage === "warm") {
      assert(
        producers.every((module) => !module.built),
        JSON.stringify(producers),
      );
    }
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
  results.checks.push({
    name: `${mode}-failure`,
    passed: false,
    error: String(error instanceof Error ? error.stack : error),
  });
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
