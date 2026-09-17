import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { runInNewContext } from "node:vm";

import { TraceMap, originalPositionFor } from "@jridgewell/trace-mapping";
import { build } from "vite-plus";

import { stillwind } from "../src/index.ts";

void test("explicit module CSS exports scoped values without adding document CSS and retains helpers", async () => {
  const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../..");
  const root = mkdtempSync(path.join(repositoryRoot, "packages/vite/.session-test-"));
  const put = (name: string, content: string) => writeFileSync(path.join(root, name), content);
  put("index.html", '<script type="module" src="/main.ts"></script>');
  put(
    "root.css",
    ".document-base{color:purple;} @utility red {color:red; background-image:url('./icon.svg');} @utility blue {color:blue;} @utility green {color:green;} @stillwind-slot utilities;",
  );
  put("icon.svg", '<svg xmlns="http://www.w3.org/2000/svg"><text>icon</text></svg>');
  put(
    "main.ts",
    `import css from 'stillwind/module-css'; import {style} from 'stillwind/react'; import {otherCss} from './other'; import {auto} from './auto'; globalThis.result = {css, otherCss, auto, props: style('red', {class: 'external'})};`,
  );
  put(
    "other.ts",
    `import otherCss from 'stillwind/module-css'; import {style} from 'stillwind/react'; export {otherCss}; export const unused = style('green');`,
  );
  put("auto.ts", `import {style} from 'stillwind/react'; export const auto=style('blue');`);
  try {
    const result = await build({
      configFile: false,
      root,
      logLevel: "silent",
      plugins: [stillwind({ css: path.join(root, "root.css"), types: false })],
      build: {
        write: false,
        minify: false,
        cssMinify: false,
        assetsInlineLimit: 0,
        modulePreload: false,
      },
    });
    if (!("output" in result)) throw new Error("expected a single build output");
    const documentCss = result.output
      .filter((asset) => asset.type === "asset")
      .filter((asset) => asset.fileName.endsWith(".css"))
      .map((asset) => String(asset.source))
      .join("\n");
    assert.match(documentCss, /color:\s*blue/);
    assert.doesNotMatch(documentCss, /color:\s*(red|green)/);
    assert.equal(documentCss.match(/\.document-base/g)?.length, 1);
    const entry = result.output.find((asset) => asset.type === "chunk" && asset.isEntry);
    assert.ok(entry?.type === "chunk");
    const global: { result?: { css: string; otherCss: string; props: { className: string } } } = {};
    runInNewContext(entry.code, { globalThis: global });
    assert.ok(global.result);
    assert.match(global.result.css, /color:\s*red/);
    assert.doesNotMatch(global.result.css, /color:\s*(blue|green)/);
    assert.match(global.result.otherCss, /color:\s*green/);
    assert.doesNotMatch(global.result.otherCss, /color:\s*(blue|red)/);
    assert.match(global.result.css, /\.document-base/);
    assert.match(global.result.css, /assets\/icon-[\w-]+\.svg/);
    assert.doesNotMatch(global.result.css, /@fs|__stillwind_resource/);
    assert.match(global.result.props.className, /external/);
    assert.match(global.result.props.className, /sw_/);
    assert.equal(
      result.output.filter((asset) => asset.type === "asset" && asset.fileName.endsWith(".svg"))
        .length,
      1,
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

void test("cross-module composition is accepted and a value carries only its own rules", async () => {
  const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../..");
  const root = mkdtempSync(path.join(repositoryRoot, "packages/vite/.session-test-"));
  const put = (name: string, content: string) => writeFileSync(path.join(root, name), content);
  put("index.html", '<script type="module" src="/main.ts"></script>');
  put(
    "root.css",
    "@utility red { color: red; } @utility blue { color: blue; } @stillwind-slot utilities;",
  );
  put("base.ts", "import { style } from 'stillwind/react'; export const base = style('red');");
  put(
    "main.ts",
    `import css from 'stillwind/module-css';
import { style } from 'stillwind/react';
import { base } from './base';
globalThis.value = css;
globalThis.props = style(base, 'blue');
`,
  );
  try {
    const result = await build({
      configFile: false,
      root,
      logLevel: "silent",
      plugins: [stillwind({ css: path.join(root, "root.css"), types: false })],
      build: { write: false, minify: false, cssMinify: false, modulePreload: false },
    });
    if (!("output" in result)) throw new Error("expected a single build output");
    const entry = result.output.find((asset) => asset.type === "chunk" && asset.isEntry);
    if (!entry || entry.type !== "chunk") throw new Error("entry chunk not found");
    const global: { value?: string; props?: { className: string } } = {};
    runInNewContext(entry.code, { globalThis: global });
    assert.match(global.value ?? "", /color:\s*blue/);
    assert.doesNotMatch(global.value ?? "", /color:\s*red/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

void test("relative JS resources preserve distinct producer origins", async () => {
  const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../..");
  const root = mkdtempSync(path.join(repositoryRoot, ".session-test-"));
  const put = (name: string, content: string) => writeFileSync(path.join(root, name), content);
  mkdirSync(path.join(root, "a"));
  mkdirSync(path.join(root, "b"));
  put("index.html", '<script type="module" src="/main.ts"></script>');
  put("root.css", '@import "../packages/core/index.css";');
  put("main.ts", 'import {a} from "./a/style"; import {b} from "./b/style"; console.log(a,b);');
  for (const name of ["a", "b"]) {
    put(`${name}/icon.svg`, `<svg xmlns="http://www.w3.org/2000/svg"><text>${name}</text></svg>`);
    put(
      `${name}/style.ts`,
      `import {style} from "stillwind/react"; export const ${name}=style("bg-[url(./icon.svg)]");`,
    );
  }
  try {
    const result = await build({
      configFile: false,
      root,
      logLevel: "silent",
      plugins: [stillwind({ css: path.join(root, "root.css"), types: false })],
      build: { write: false, minify: false, cssMinify: false, assetsInlineLimit: 0 },
    });
    if (!("output" in result)) throw new Error("expected a single build output");
    const css = result.output
      .filter((asset) => asset.type === "asset")
      .filter((asset) => asset.fileName.endsWith(".css"))
      .map((asset) => String(asset.source))
      .join("\n");
    const urls = [...css.matchAll(/url\([^)]*icon-([\w-]+)\.svg[^)]*\)/g)];
    assert.equal(new Set(urls.map((match) => match[1])).size, 2, css);
    const classes = [...css.matchAll(/\.(sw_[0-9a-z]+)\s*\{/g)];
    assert.equal(new Set(classes.map((match) => match[1])).size, 2, css);
    assert.doesNotMatch(css, /__stillwind_|@fs|\.\/icon\.svg/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

void test("build retains late and lazy producer CSS without collecting transforms in CSS load", async () => {
  const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../..");
  const root = mkdtempSync(path.join(repositoryRoot, ".session-test-"));
  const put = (name: string, content: string) => writeFileSync(path.join(root, name), content);
  put("index.html", '<script type="module" src="/main.ts"></script>');
  put("root.css", ".document-only{color:purple;} @layer utilities { @stillwind-slot utilities; }");
  put(
    "main.ts",
    'import "virtual:stillwind.css"; import { a } from "./slow"; console.log(a); window.loadLazy = () => import("./lazy");',
  );
  put(
    "slow.ts",
    'import { style } from "stillwind/react"; export const a = style("[color:rgb(11,22,33)]");',
  );
  put(
    "lazy.ts",
    'import { style } from "stillwind/react"; export const b = style("[color:rgb(44,55,66)]");',
  );
  put(
    "unused.ts",
    'import { style } from "stillwind/react"; export const c = style("[color:rgb(77,88,99)]");',
  );
  try {
    const output = await build({
      configFile: false,
      root,
      logLevel: "silent",
      plugins: [
        {
          name: "delay-producer",
          enforce: "pre",
          async transform(_, id) {
            if (id.endsWith("/slow.ts")) await new Promise((resolve) => setTimeout(resolve, 200));
          },
        },
        stillwind({ css: path.join(root, "root.css"), types: false }),
      ],
      build: { write: false, minify: false, cssMinify: false, sourcemap: true },
    });
    if (!("output" in output)) throw new Error("expected a single build output");
    const assets = output.output
      .filter((asset) => asset.type === "asset")
      .filter((asset) => asset.fileName.endsWith(".css"));
    const css = assets.map((asset) => String(asset.source)).join("\n");
    assert.match(css, /11,\s*22,\s*33/);
    assert.match(css, /44,\s*55,\s*66/);
    assert.doesNotMatch(css, /77,\s*88,\s*99/);
    assert.equal(css.match(/\.document-only/g)?.length, 1);
    assert.ok(assets.some((asset) => asset.fileName.includes("lazy")));
    // Vite currently emits JS build maps but discards CSS build maps. Keep the
    // boundary explicit; the adapter's CSS maps are accepted in development.
    assert.equal(
      output.output.some((asset) => asset.fileName.endsWith(".css.map")),
      false,
    );
    const lazy = output.output.find(
      (asset) => asset.type === "chunk" && asset.fileName.includes("lazy"),
    );
    assert.ok(lazy?.type === "chunk" && lazy.map);
    const classOffset = lazy.code.indexOf("sw_");
    assert.ok(classOffset >= 0);
    const beforeClass = lazy.code.slice(0, classOffset);
    const position = originalPositionFor(new TraceMap(lazy.map.toString()), {
      line: beforeClass.split("\n").length,
      column: beforeClass.length - beforeClass.lastIndexOf("\n") - 1,
    });
    assert.ok(position.source?.endsWith("lazy.ts"));
    assert.equal(position.line, 1);
    assert.equal(
      position.column,
      'import { style } from "stillwind/react"; export const b = '.length,
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

void test("imported CSS resources retain their owner through document and module carriers", async () => {
  const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../..");
  const root = mkdtempSync(path.join(repositoryRoot, ".session-test-"));
  const put = (name: string, content: string) => writeFileSync(path.join(root, name), content);
  mkdirSync(path.join(root, "nested"));
  put("index.html", '<script type="module" src="/main.ts"></script>');
  put("root.css", '@import "./nested/design.css"; @stillwind-slot utilities;');
  put(
    "nested/design.css",
    '.document-only{background:url("./document.svg")} @stillwind-utility picture{background-image:url("./utility.svg")}',
  );
  put(
    "nested/document.svg",
    '<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><path fill="red" d="M0 0h1v1H0z"/></svg>',
  );
  put(
    "nested/utility.svg",
    '<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><path fill="blue" d="M0 0h2v2H0z"/></svg>',
  );
  put(
    "main.ts",
    'import "virtual:stillwind.css"; import { style } from "stillwind/react"; console.log(style("picture"));',
  );
  try {
    const result = await build({
      configFile: false,
      root,
      logLevel: "silent",
      plugins: [stillwind({ css: path.join(root, "root.css"), types: false })],
      build: { write: false, minify: false, cssMinify: false, assetsInlineLimit: 0 },
    });
    if (!("output" in result)) throw new Error("expected a single build output");
    const css = result.output
      .filter((asset) => asset.type === "asset")
      .filter((asset) => asset.fileName.endsWith(".css"))
      .map((asset) => String(asset.source))
      .join("\n");
    assert.match(css, /assets\/document-[\w-]+\.svg/);
    assert.match(css, /assets\/utility-[\w-]+\.svg/);
    assert.doesNotMatch(css, /__stillwind_resource_|@fs|nested\//);
    assert.equal(
      result.output.filter((asset) => asset.type === "asset" && asset.fileName.endsWith(".svg"))
        .length,
      2,
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
