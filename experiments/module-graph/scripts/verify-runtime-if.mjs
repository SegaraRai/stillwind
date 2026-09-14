// Browser acceptance for computed-value conditions in both compiler output modes.
import assert from "node:assert/strict";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { chromium } from "playwright";
import { build, preview } from "vite-plus";

import { stillwind } from "../../../packages/vite/src/index.ts";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const output = path.join(root, "output");
await fs.mkdir(output, { recursive: true });
const dir = await fs.mkdtemp(path.join(output, "runtime-if-"));
const core = path
  .relative(dir, path.resolve(root, "../../packages/core/index.css"))
  .replaceAll("\\", "/");
await fs.writeFile(path.join(dir, "root.css"), `@import ${JSON.stringify(core)};`);
await fs.writeFile(path.join(dir, "index.html"), '<script type="module" src="/main.ts"></script>');
await fs.writeFile(
  path.join(dir, "main.ts"),
  `
import {style} from 'stillwind/react';
const props = style('grid if(--enabled):flex w-[--if(--enabled,120px,60px)] h-[--if(--enabled,--if(--second,30px,20px),10px)] opacity-100 if(--enabled):if(--second):opacity-50! before:if(--enabled):content-["yes"]', {'--enabled': false, '--second': true});
const element = document.createElement('div');
element.id = 'sample'; element.className = props.className;
for (const [name, value] of Object.entries(props.style)) element.style.setProperty(name, String(value));
document.body.append(element);
const stacked = style('grid if(--enabled):block if(--second):flex');
const child = document.createElement('div'); child.id = 'stacked'; child.className = stacked.className;
element.append(child);
const inverted = style('__NEGATED_VARIANTS__ w-[--if(!--enabled,40px,80px)]');
const inverse = document.createElement('div'); inverse.id = 'inverse'; inverse.className = inverted.className;
element.append(inverse);
const compared = style('__COMPARISON_UTILITIES__');
const comparison = document.createElement('div'); comparison.id = 'comparison'; comparison.className = compared.className;
element.append(comparison);
for (const item of [child, inverse, comparison]) item.style.flexShrink = '0';
`,
);
const originalSource = await fs.readFile(path.join(dir, "main.ts"), "utf8");
const browser = await chromium.launch({ channel: "chrome", headless: true });
try {
  const page = await browser.newPage();
  const support = await page.evaluate(() => ({
    revertRule: CSS.supports("display", "revert-rule"),
    cssIf: CSS.supports("width", "if(style(--x):1px;else:2px)"),
  }));
  console.log(JSON.stringify({ browser: browser.version(), support }));
  assert.ok(support.revertRule, "Browser acceptance requires revert-rule support");
  assert.ok(support.cssIf, "Browser acceptance requires CSS if() support");
  for (const useCssIf of [false, true]) {
    await fs.writeFile(
      path.join(dir, "main.ts"),
      originalSource
        .replace(
          "__NEGATED_VARIANTS__",
          useCssIf ? "grid if(!--enabled):flex if(!--enabled):if(--second):opacity-50" : "grid",
        )
        .replace(
          "__COMPARISON_UTILITIES__",
          useCssIf
            ? "grid if(--mode:compact):flex w-[--if(--mode:compact,40px,80px)] if(--mode:compact):if(--second):opacity-50"
            : "grid w-[80px]",
        ),
    );
    await build({
      configFile: false,
      root: dir,
      logLevel: "silent",
      plugins: [stillwind({ css: path.join(dir, "root.css"), types: false, useCssIf })],
      build: { minify: true, cssMinify: true, outDir: `dist-${useCssIf}` },
    });
    const server = await preview({
      configFile: false,
      root: dir,
      logLevel: "silent",
      build: { outDir: `dist-${useCssIf}` },
      preview: { port: 0 },
    });
    try {
      await page.goto(server.resolvedUrls.local[0]);
      await page.waitForSelector("#sample");
      for (const [enabled, second] of [
        [false, false],
        [true, false],
        [true, true],
        [false, true],
      ]) {
        const actual = await page.evaluate(
          ({ enabled, second }) => {
            const element = document.querySelector("#sample");
            element.style.setProperty("--enabled", enabled ? " " : "initial");
            element.style.setProperty("--second", second ? " " : "initial");
            element.style.setProperty("--mode", enabled ? "compact" : "expanded");
            const style = getComputedStyle(element);
            return {
              display: style.display,
              width: style.width,
              height: style.height,
              opacity: style.opacity,
              stacked: getComputedStyle(document.querySelector("#stacked")).display,
              before: getComputedStyle(element, "::before").content,
              inverseWidth: getComputedStyle(document.querySelector("#inverse")).width,
              inverseDisplay: getComputedStyle(document.querySelector("#inverse")).display,
              inverseOpacity: getComputedStyle(document.querySelector("#inverse")).opacity,
              comparisonWidth: getComputedStyle(document.querySelector("#comparison")).width,
              comparisonDisplay: getComputedStyle(document.querySelector("#comparison")).display,
              comparisonOpacity: getComputedStyle(document.querySelector("#comparison")).opacity,
            };
          },
          { enabled, second },
        );
        assert.deepEqual(
          actual,
          {
            display: enabled ? "flex" : "grid",
            width: enabled ? "120px" : "60px",
            height: enabled ? (second ? "30px" : "20px") : "10px",
            opacity: enabled && second ? "0.5" : "1",
            stacked: second ? "flex" : enabled ? "block" : "grid",
            before: enabled ? '"yes"' : "none",
            inverseWidth: enabled ? "80px" : "40px",
            inverseDisplay: useCssIf && !enabled ? "flex" : "grid",
            inverseOpacity: useCssIf && !enabled && second ? "0.5" : "1",
            comparisonWidth: useCssIf && enabled ? "40px" : "80px",
            comparisonDisplay: useCssIf && enabled ? "flex" : "grid",
            comparisonOpacity: useCssIf && enabled && second ? "0.5" : "1",
          },
          JSON.stringify({ useCssIf, enabled, second }),
        );
      }
      console.log(
        `useCssIf=${useCssIf}: production minification and all four boolean states passed`,
      );
    } finally {
      await new Promise((resolve, reject) =>
        server.httpServer.close((error) => (error ? reject(error) : resolve())),
      );
    }
  }
} finally {
  await browser.close();
}
