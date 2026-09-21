import assert from "node:assert/strict";
import { runInNewContext } from "node:vm";

import { chromium } from "playwright";

import { compileSource } from "../../../packages/native/index.js";

const output = compileSource(
  "import {style} from 'stillwind/react'; const fade = style.animation({from:'a', to:'b', 'motion-reduce:': {to:'c'}}); export const props = style(fade(''));",
  "animation.tsx",
  "@utility a {opacity:0;} @utility b {opacity:1;} @utility c {opacity:.5;} @variant motion-reduce {@media (prefers-reduced-motion:reduce) {@slot;}} @stillwind-slot utilities; @stillwind-slot keyframes;",
);
assert.deepEqual(output.diagnostics, []);
const sandbox = {};
runInNewContext(output.code.replace("export const props", "globalThis.props"), sandbox);
const props = sandbox.props;
const browser = await chromium.launch({ channel: "chrome", headless: true });
try {
  const page = await browser.newPage();
  await page.setContent(
    `<style>${output.css} #target {opacity:.25;animation-duration:1s;animation-fill-mode:both;animation-play-state:paused;}</style><div id="target" class="${props.className}"></div>`,
  );
  for (const [reducedMotion, expected] of [
    ["no-preference", [0, 1]],
    ["reduce", [0.25, 0.5]],
  ]) {
    await page.emulateMedia({ reducedMotion });
    const values = await page.evaluate(async () => {
      await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
      const target = document.querySelector("#target");
      const animation = target.getAnimations()[0];
      animation.currentTime = 0;
      const start = Number(getComputedStyle(target).opacity);
      animation.currentTime = 1000;
      return [start, Number(getComputedStyle(target).opacity)];
    });
    assert.deepEqual(values, expected, reducedMotion);
  }
  console.log("Animation media branches replace complete keyframes in Chrome.");
} finally {
  await browser.close();
}
