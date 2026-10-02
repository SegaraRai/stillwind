import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { runInNewContext } from "node:vm";

import { TraceMap, originalPositionFor } from "@jridgewell/trace-mapping";
import { stillwind } from "@stillwind/integration/vite";
import { build } from "vite";
import { expect, test } from "vite-plus/test";

test("server-only projections emit split CSS without publishing server source contents", async () => {
  const root = mkdtempSync(
    path.join(path.dirname(fileURLToPath(import.meta.url)), "../.test-session-"),
  );
  writeFileSync(path.join(root, "index.html"), '<script type="module" src="/main.ts"></script>');
  writeFileSync(path.join(root, "main.ts"), "import './server.ts?stillwind.css';");
  writeFileSync(
    path.join(root, "server.ts"),
    "import {style} from 'stillwind/react';const privateValue='SERVER_SOURCE_SENTINEL';export const props=style('red');",
  );
  writeFileSync(path.join(root, "root.css"), "@utility red{color:red}@stillwind-slot utilities;");
  try {
    const result = await build({
      configFile: false,
      root,
      logLevel: "silent",
      plugins: [
        stillwind({
          css: "./root.css",
          types: false,
          emission: "split",
        }),
      ],
      build: {
        write: false,
        minify: false,
        sourcemap: true,
      },
    });
    if (!("output" in result)) {
      expect.fail("expected output");
    }
    const code = result.output
      .filter((item) => item.type === "chunk")
      .map((item) => item.code)
      .join("");
    expect(code).toMatch(/data-stillwind-part/);
    expect(result.output.some((item) => item.fileName.endsWith(".css"))).toBeTruthy();
    for (const item of result.output) {
      expect(item.type === "chunk" ? item.code : String(item.source)).not.toMatch(
        /SERVER_SOURCE_SENTINEL/,
      );
    }
  } finally {
    expect(
      path
        .resolve(root)
        .startsWith(path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..") + path.sep),
    ).toBeTruthy();
    rmSync(root, { recursive: true, force: true });
  }
});

test("explicit module CSS exports scoped values without adding document CSS and retains helpers", async () => {
  const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../../..");
  const root = mkdtempSync(path.join(repositoryRoot, "packages/integration/.test-session-"));
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
      plugins: [
        stillwind({
          css: path.join(root, "root.css"),
          types: false,
        }),
      ],
      build: {
        write: false,
        minify: false,
        cssMinify: false,
        assetsInlineLimit: 0,
        modulePreload: false,
      },
    });
    if (!("output" in result)) {
      expect.fail("expected a single build output");
    }
    const documentCss = result.output
      .filter((asset) => asset.type === "asset")
      .filter((asset) => asset.fileName.endsWith(".css"))
      .map((asset) => String(asset.source))
      .join("\n");
    expect(documentCss).toMatch(/color:\s*blue/);
    expect(documentCss).not.toMatch(/color:\s*(red|green)/);
    expect(documentCss.match(/\.document-base/g)?.length).toBe(1);
    const entry = result.output.find((asset) => asset.type === "chunk" && asset.isEntry);
    if (!(entry?.type === "chunk")) {
      expect.fail('Unexpected value: entry?.type === "chunk"');
    }
    const global: {
      result?: {
        css: string;
        otherCss: string;
        props: {
          className: string;
        };
      };
    } = {};
    runInNewContext(entry!.code, { globalThis: global });
    expect(global.result).toBeTruthy();
    expect(global.result!.css).toMatch(/color:\s*red/);
    expect(global.result!.css).not.toMatch(/color:\s*(blue|green)/);
    expect(global.result!.otherCss).toMatch(/color:\s*green/);
    expect(global.result!.otherCss).not.toMatch(/color:\s*(blue|red)/);
    expect(global.result!.css).toMatch(/\.document-base/);
    expect(global.result!.css).toMatch(/assets\/icon-[\w-]+\.svg/);
    expect(global.result!.css).not.toMatch(/@fs|__stillwind_resource/);
    expect(global.result!.props.className).toMatch(/external/);
    expect(global.result!.props.className).toMatch(/sw_/);
    expect(
      result.output.filter((asset) => asset.type === "asset" && asset.fileName.endsWith(".svg"))
        .length,
    ).toBe(1);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("a CSS value includes its transitive composition closure in reference order", async () => {
  const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../../..");
  const root = mkdtempSync(path.join(repositoryRoot, "packages/integration/.test-session-"));
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
      plugins: [
        stillwind({
          css: path.join(root, "root.css"),
          types: false,
        }),
      ],
      build: {
        write: false,
        minify: false,
        cssMinify: false,
        modulePreload: false,
      },
    });
    if (!("output" in result)) {
      expect.fail("expected a single build output");
    }
    const entry = result.output.find((asset) => asset.type === "chunk" && asset.isEntry);
    if (!entry || entry.type !== "chunk") {
      expect.fail("entry chunk not found");
    }
    const global: {
      value?: string;
      props?: {
        className: string;
      };
    } = {};
    runInNewContext(entry.code, { globalThis: global });
    expect(global.value ?? "").toMatch(/color:\s*blue/);
    expect(global.value ?? "").toMatch(/color:\s*red/);
    expect(global.value!.indexOf("red")).toBeLessThan(global.value!.indexOf("blue"));
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("relative JS resources preserve distinct producer origins", async () => {
  const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../../..");
  const root = mkdtempSync(path.join(repositoryRoot, ".test-session-"));
  const put = (name: string, content: string) => writeFileSync(path.join(root, name), content);
  mkdirSync(path.join(root, "a"));
  mkdirSync(path.join(root, "b"));
  put("index.html", '<script type="module" src="/main.ts"></script>');
  put("root.css", '@import "../packages/core/index.css";');
  put(
    "main.ts",
    'import { a } from "./a/style"; import { b } from "./b/style"; console.log(a,  b);',
  );
  for (const name of ["a", "b"]) {
    put(`${name}/icon.svg`, `<svg xmlns="http://www.w3.org/2000/svg"><text>${name}</text></svg>`);
    put(
      `${name}/style.ts`,
      `import { style } from "stillwind/react"; export const ${name} = style("bg-[url(./icon.svg)]");`,
    );
  }
  try {
    const result = await build({
      configFile: false,
      root,
      logLevel: "silent",
      plugins: [
        stillwind({
          css: path.join(root, "root.css"),
          types: false,
        }),
      ],
      build: {
        write: false,
        minify: false,
        cssMinify: false,
        assetsInlineLimit: 0,
      },
    });
    if (!("output" in result)) {
      expect.fail("expected a single build output");
    }
    const css = result.output
      .filter((asset) => asset.type === "asset")
      .filter((asset) => asset.fileName.endsWith(".css"))
      .map((asset) => String(asset.source))
      .join("\n");
    const urls = [...css.matchAll(/url\([^)]*icon-([\w-]+)\.svg[^)]*\)/g)];
    expect(new Set(urls.map((match) => match[1])).size, css).toBe(2);
    const classes = [...css.matchAll(/\.(sw_[0-9a-z]+)\s*\{/g)];
    expect(new Set(classes.map((match) => match[1])).size, css).toBe(2);
    expect(css).not.toMatch(/__stillwind_|@fs|\.\/icon\.svg/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("build retains late and lazy producer CSS without collecting transforms in CSS load", async () => {
  const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../../..");
  const root = mkdtempSync(path.join(repositoryRoot, ".test-session-"));
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
            if (id.endsWith("/slow.ts")) {
              await new Promise((resolve) => setTimeout(resolve, 200));
            }
          },
        },
        stillwind({
          css: path.join(root, "root.css"),
          types: false,
        }),
      ],
      build: {
        write: false,
        minify: false,
        cssMinify: false,
        sourcemap: true,
      },
    });
    if (!("output" in output)) {
      expect.fail("expected a single build output");
    }
    const assets = output.output
      .filter((asset) => asset.type === "asset")
      .filter((asset) => asset.fileName.endsWith(".css"));
    const css = assets.map((asset) => String(asset.source)).join("\n");
    expect(css).toMatch(/11,\s*22,\s*33/);
    expect(css).toMatch(/44,\s*55,\s*66/);
    expect(css).not.toMatch(/77,\s*88,\s*99/);
    expect(css.match(/\.document-only/g)?.length).toBe(1);
    expect(assets.length).toBe(1);
    expect(output.output.some((asset) => asset.fileName.endsWith(".css.map"))).toBeTruthy();
    const lazy = output.output.find(
      (asset) => asset.type === "chunk" && asset.fileName.includes("lazy"),
    );
    if (!(lazy?.type === "chunk" && lazy.map)) {
      expect.fail('Unexpected value: lazy?.type === "chunk" && lazy.map');
    }
    const classOffset = lazy!.code.indexOf("sw_");
    expect(classOffset).toBeGreaterThanOrEqual(0);
    const beforeClass = lazy!.code.slice(0, classOffset);
    const position = originalPositionFor(new TraceMap(lazy!.map.toString()), {
      line: beforeClass.split("\n").length,
      column: beforeClass.length - beforeClass.lastIndexOf("\n") - 1,
    });
    expect(position.source?.endsWith("lazy.ts")).toBeTruthy();
    expect(position.line).toBe(1);
    expect(position.column).toBe(
      'import { style } from "stillwind/react"; export const b = '.length,
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("imported CSS resources retain their owner through document and module carriers", async () => {
  const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../../..");
  const root = mkdtempSync(path.join(repositoryRoot, ".test-session-"));
  const put = (name: string, content: string) => writeFileSync(path.join(root, name), content);
  mkdirSync(path.join(root, "nested folder"));
  put("index.html", '<script type="module" src="/main.ts"></script>');
  put("root.css", '@import "./nested folder/design.css"; @stillwind-slot utilities;');
  put(
    "nested folder/design.css",
    '.document-only{background:url("./document.svg")} @stillwind-utility picture{background-image:url("./utility.svg")}',
  );
  put(
    "nested folder/document.svg",
    '<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><path fill="red" d="M0 0h1v1H0z"/></svg>',
  );
  put(
    "nested folder/utility.svg",
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
      plugins: [
        stillwind({
          css: path.join(root, "root.css"),
          types: false,
        }),
      ],
      build: {
        write: false,
        minify: false,
        cssMinify: false,
        assetsInlineLimit: 0,
      },
    });
    if (!("output" in result)) {
      expect.fail("expected a single build output");
    }
    const css = result.output
      .filter((asset) => asset.type === "asset")
      .filter((asset) => asset.fileName.endsWith(".css"))
      .map((asset) => String(asset.source))
      .join("\n");
    expect(css).toMatch(/document-[\w-]+\.svg/);
    expect(css).toMatch(/utility-[\w-]+\.svg/);
    expect(css).not.toMatch(/__stillwind_resource_|@fs|nested\//);
    expect(
      result.output.filter((asset) => asset.type === "asset" && asset.fileName.endsWith(".svg"))
        .length,
    ).toBe(2);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
