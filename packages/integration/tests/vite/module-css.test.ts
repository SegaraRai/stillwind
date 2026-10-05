import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { runInNewContext } from "node:vm";

import { TraceMap, originalPositionFor } from "@jridgewell/trace-mapping";
import { stillwind } from "@stillwind/integration/vite";
import { build, type Plugin } from "vite";
import { expect, onTestFinished, test } from "vite-plus/test";

test("importing server CSS emits split styles without including the server source", async () => {
  const output = await buildProject(
    {
      "main.ts": "import './server.ts?stillwind.css';",
      "server.ts": `
import { style } from 'stillwind/react';
const secret = 'SERVER_SOURCE_SENTINEL';
export const props = style('red');
`,
      "root.css": "@utility red { color: red; } @stillwind-slot utilities;",
    },
    { emission: "split", sourcemap: true },
  );

  const javascript = output
    .filter((item) => item.type === "chunk")
    .map((chunk) => chunk.code)
    .join("\n");
  expect(javascript).toContain("data-stillwind-part");
  expect(cssAssets(output).length).toBeGreaterThan(0);
  for (const item of output) {
    const publicText = item.type === "chunk" ? item.code : String(item.source);
    expect(publicText).not.toContain("SERVER_SOURCE_SENTINEL");
  }
});

test("CSS values contain each module's styles while automatic styles go to document CSS", async () => {
  const { output, values } = await buildCssValues();
  const documentCss = joinCss(output);

  expect(documentCss).toMatch(/color:\s*blue/);
  expect(documentCss).not.toMatch(/color:\s*(red|green)/);
  expect(documentCss.match(/\.document-base/g)).toHaveLength(1);
  expect(values.css).toMatch(/color:\s*red/);
  expect(values.css).not.toMatch(/color:\s*(blue|green)/);
  expect(values.otherCss).toMatch(/color:\s*green/);
  expect(values.otherCss).not.toMatch(/color:\s*(blue|red)/);
});

test("a CSS value includes document rules and resolves its image to an emitted asset", async () => {
  const { output, values } = await buildCssValues();

  expect(values.css).toContain(".document-base");
  expect(values.css).toMatch(/assets\/icon-[\w-]+\.svg/);
  expect(values.css).not.toMatch(/@fs|__stillwind_resource/);
  expect(
    output.filter((item) => item.type === "asset" && item.fileName.endsWith(".svg")),
  ).toHaveLength(1);
});

test("requesting a CSS value keeps the runtime helper that merges external classes", async () => {
  const { values } = await buildCssValues();

  expect(values.props.className).toContain("external");
  expect(values.props.className).toContain("sw_");
});

test("a CSS value includes imported styles before local styles", async () => {
  const output = await buildProject({
    "root.css":
      "@utility red { color: red; } @utility blue { color: blue; } @stillwind-slot utilities;",
    "base.ts": "import { style } from 'stillwind/react'; export const base = style('red');",
    "main.ts": `
import css from 'stillwind/module-css';
import { style } from 'stillwind/react';
import { base } from './base';
globalThis.value = css;
globalThis.props = style(base, 'blue');
`,
  });
  const globals: { value?: string } = {};
  runInNewContext(entryChunk(output).code, {
    globalThis: globals,
  });
  if (globals.value === undefined) {
    expect.fail("Expected main.ts to export its CSS value");
  }
  const cssValue = globals.value;

  expect(cssValue).toMatch(/color:\s*red/);
  expect(cssValue).toMatch(/color:\s*blue/);
  expect(cssValue.indexOf("red")).toBeLessThan(cssValue.indexOf("blue"));
});

test("the same relative image URL in two modules resolves from each module's directory", async () => {
  const output = await buildProject({
    "root.css": '@import "../../core/index.css";',
    "main.ts": `
import { a } from './a/style';
import { b } from './b/style';
console.log(a, b);
`,
    "a/icon.svg": '<svg xmlns="http://www.w3.org/2000/svg"><text>a</text></svg>',
    "b/icon.svg": '<svg xmlns="http://www.w3.org/2000/svg"><text>b</text></svg>',
    "a/style.ts": `import { style } from 'stillwind/react'; export const a = style('bg-[url(./icon.svg)]');`,
    "b/style.ts": `import { style } from 'stillwind/react'; export const b = style('bg-[url(./icon.svg)]');`,
  });
  const css = joinCss(output);
  const imageNames = [...css.matchAll(/url\([^)]*icon-([\w-]+)\.svg[^)]*\)/g)].map(
    (match) => match[1],
  );
  const classNames = [...css.matchAll(/\.(sw_[0-9a-z]+)\s*\{/g)].map((match) => match[1]);

  expect(new Set(imageNames).size, css).toBe(2);
  expect(new Set(classNames).size, css).toBe(2);
  expect(css).not.toMatch(/__stillwind_|@fs|\.\/icon\.svg/);
});

test("document CSS includes delayed and lazy imports and excludes unused modules", async () => {
  const output = await buildLazyStyles();
  const css = joinCss(output);

  expect(css).toMatch(/11,\s*22,\s*33/);
  expect(css).toMatch(/44,\s*55,\s*66/);
  expect(css).not.toMatch(/77,\s*88,\s*99/);
  expect(css.match(/\.document-only/g)).toHaveLength(1);
  expect(cssAssets(output)).toHaveLength(1);
  expect(output.map((item) => item.fileName)).toContainEqual(expect.stringMatching(/\.css\.map$/));
});

test("a lazy module's generated class maps to its original style call", async () => {
  const output = await buildLazyStyles();
  const lazyChunk = output.find((item) => item.type === "chunk" && item.fileName.includes("lazy"));
  if (!lazyChunk || lazyChunk.type !== "chunk" || !lazyChunk.map) {
    expect.fail("Expected the lazy JavaScript chunk with a source map");
  }
  const classOffset = lazyChunk.code.indexOf("sw_");
  expect(classOffset).toBeGreaterThanOrEqual(0);
  const prefix = lazyChunk.code.slice(0, classOffset);

  const original = originalPositionFor(new TraceMap(lazyChunk.map.toString()), {
    line: prefix.split("\n").length,
    column: classOffset - prefix.lastIndexOf("\n") - 1,
  });

  expect(original).toMatchObject({
    source: expect.stringMatching(/lazy\.ts$/),
    line: 1,
    column: lazyStyleSource.indexOf('style("[color:'),
  });
});

test("image URLs in imported CSS resolve from the CSS file's directory, including spaces", async () => {
  const output = await buildProject({
    "root.css": '@import "./nested folder/design.css"; @stillwind-slot utilities;',
    "nested folder/design.css": `
.document-only { background: url('./document.svg'); }
@stillwind-utility picture { background-image: url('./utility.svg'); }
`,
    "nested folder/document.svg":
      '<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><path fill="red" d="M0 0h1v1H0z"/></svg>',
    "nested folder/utility.svg":
      '<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><path fill="blue" d="M0 0h2v2H0z"/></svg>',
    "main.ts": `
import 'virtual:stillwind.css';
import { style } from 'stillwind/react';
console.log(style('picture'));
`,
  });
  const css = joinCss(output);

  expect(css).toMatch(/document-[\w-]+\.svg/);
  expect(css).toMatch(/utility-[\w-]+\.svg/);
  expect(css).not.toMatch(/__stillwind_resource_|@fs|nested\//);
  expect(
    output.filter((item) => item.type === "asset" && item.fileName.endsWith(".svg")),
  ).toHaveLength(2);
});

async function buildProject(
  files: Record<string, string>,
  options: { emission?: "single" | "split"; sourcemap?: boolean; plugins?: Plugin[] } = {},
) {
  const packageRoot = fileURLToPath(new URL("../../", import.meta.url));
  const root = mkdtempSync(path.join(packageRoot, ".test-module-css-"));
  onTestFinished(() => rmSync(root, { recursive: true, force: true }));
  const allFiles = { "index.html": '<script type="module" src="/main.ts"></script>', ...files };
  for (const [filename, content] of Object.entries(allFiles)) {
    const file = path.join(root, filename);
    mkdirSync(path.dirname(file), { recursive: true });
    writeFileSync(file, content);
  }
  const result = await build({
    configFile: false,
    root,
    logLevel: "silent",
    plugins: [
      ...(options.plugins ?? []),
      stillwind({
        css: path.join(root, "root.css"),
        types: false,
        emission: options.emission,
      }),
    ],
    build: {
      write: false,
      minify: false,
      cssMinify: false,
      assetsInlineLimit: 0,
      modulePreload: false,
      sourcemap: options.sourcemap ?? false,
    },
  });
  if (Array.isArray(result) || !("output" in result)) {
    expect.fail("Expected one completed build");
  }
  return result.output;
}

type BuildOutput = Awaited<ReturnType<typeof buildProject>>;

function cssAssets(output: BuildOutput) {
  return output
    .filter((item) => item.type === "asset")
    .filter((asset) => asset.fileName.endsWith(".css"));
}

function joinCss(output: BuildOutput) {
  return cssAssets(output)
    .map((asset) => String(asset.source))
    .join("\n");
}

function entryChunk(output: BuildOutput) {
  const entry = output.find((item) => item.type === "chunk" && item.isEntry);
  if (!entry || entry.type !== "chunk") {
    expect.fail("Expected the main JavaScript entry chunk");
  }
  return entry;
}

async function buildCssValues() {
  const output = await buildProject({
    "root.css": `
.document-base { color: purple; }
@utility red { color: red; background-image: url('./icon.svg'); }
@utility blue { color: blue; }
@utility green { color: green; }
@stillwind-slot utilities;
`,
    "icon.svg": '<svg xmlns="http://www.w3.org/2000/svg"><text>icon</text></svg>',
    "main.ts": `
import css from 'stillwind/module-css';
import { style } from 'stillwind/react';
import { otherCss } from './other';
import { auto } from './auto';
globalThis.result = { css, otherCss, auto, props: style('red', { class: 'external' }) };
`,
    "other.ts": `
import otherCss from 'stillwind/module-css';
import { style } from 'stillwind/react';
export { otherCss };
export const unused = style('green');
`,
    "auto.ts": "import { style } from 'stillwind/react'; export const auto = style('blue');",
  });
  const globals: {
    result?: {
      css: string;
      otherCss: string;
      props: {
        className: string;
      };
    };
  } = {};
  runInNewContext(entryChunk(output).code, {
    globalThis: globals,
  });
  if (!globals.result) {
    expect.fail("Expected main.ts to export its CSS values and props");
  }
  return {
    output,
    values: globals.result,
  };
}

const lazyStyleSource =
  'import { style } from "stillwind/react"; export const b = style("[color:rgb(44,55,66)]");';

async function buildLazyStyles() {
  return buildProject(
    {
      "root.css": ".document-only{color:purple;} @layer utilities { @stillwind-slot utilities; }",
      "main.ts": `
import 'virtual:stillwind.css';
import { a } from './slow';
console.log(a);
window.loadLazy = () => import('./lazy');
`,
      "slow.ts":
        'import { style } from "stillwind/react"; export const a = style("[color:rgb(11,22,33)]");',
      "lazy.ts": lazyStyleSource,
      "unused.ts":
        'import { style } from "stillwind/react"; export const c = style("[color:rgb(77,88,99)]");',
    },
    {
      sourcemap: true,
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
      ],
    },
  );
}
