import { runInNewContext } from "node:vm";

import type { ScenarioContext } from "@stillwind-internal/e2e-harness/scenario";
import { compileSource } from "@stillwind/native";
import { chromium, type Browser } from "playwright";
import { expect } from "vite-plus/test";

/**
 * Creates lifecycle and fixture operations; initialize must run before browser contracts.
 */
export function createApplication(_context: ScenarioContext) {
  let state!: Awaited<ReturnType<typeof prepare>>;
  let browser!: Browser;
  async function initialize() {
    state = await prepare();
    browser = await chromium.launch({
      channel: "chrome",
      headless: true,
    });
  }
  async function dispose() {
    await browser?.close();
  }
  return {
    get state() {
      return state;
    },
    get browser() {
      return browser;
    },
    initialize,
    dispose,
  };
}

export type AnimationApplication = ReturnType<typeof createApplication>;

async function prepare() {
  const source =
    "import {style} from 'stillwind/react'; const fade = style.keyframes({from:'a', to:'b', 'motion-reduce:': {to:'c'}}); export const props = style(fade(''));";
  const theme =
    "@utility a {opacity:0;} @utility b {opacity:1;} @utility c {opacity:.5;} @variant motion-reduce {@media (prefers-reduced-motion:reduce) {@slot;}} @stillwind-slot utilities; @stillwind-slot keyframes;";
  const output = compileSource(
    {
      source,
      moduleId: "animation.tsx",
      uri: "untitled:animation.tsx",
      sourceMapSource: "untitled:animation.tsx",
      language: "tsx",
      moduleKind: "unambiguous",
      css: {
        content: theme,
        name: "root.css",
        uri: "untitled:root.css",
        sourceMapSource: "untitled:root.css",
      },
    },
    {},
  );
  expect(output.diagnostics).toEqual([]);
  expect(output.code).toBeTruthy();

  const sandbox = {} as {
    props: {
      className: string;
    };
  };
  runInNewContext(output.code!.replace("export const props", "globalThis.props"), sandbox);
  const props = sandbox.props;

  return {
    output,
    props,
  };
}
