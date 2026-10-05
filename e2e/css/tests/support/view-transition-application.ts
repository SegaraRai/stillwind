import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { runInNewContext } from "node:vm";

import type { ScenarioContext } from "@stillwind-internal/e2e-harness/scenario";
import { compileSource } from "@stillwind/native";
import { chromium, type Browser, type Page } from "playwright";
import * as runtime from "stillwind/react/runtime";
import { expect } from "vite-plus/test";

type CompiledProps = {
  className: string;
  style?: Record<string, string>;
};

/**
 * Creates lifecycle and fixture operations; initialize must run before browser contracts
 */
export function createApplication(_context: ScenarioContext) {
  let state!: Awaited<ReturnType<typeof prepare>>;
  let browser!: Browser;
  let page!: Page;
  async function initialize() {
    state = await prepare();
    browser = await chromium.launch({
      channel: "chrome",
      headless: true,
    });
  }

  async function initializePage() {
    const { output, props } = state;
    page = await browser.newPage();
    await page.setContent(
      `<style>${output.css}</style>` +
        (
          [
            ["static", props.staticallyKeyed],
            ["dynamic", props.dynamicallyKeyed],
            ["unioned", props.unioned],
            ["guarded", props.guarded],
            ["guardedTogether", props.guardedTogether],
            ["unnamed", props.unnamed],
          ] as const
        )
          .map(([id, value]) => `<div id="${id}" class="${value.className}"></div>`)
          .join(""),
    );
    await page.evaluate((value: CompiledProps) => {
      const target = document.querySelector<HTMLElement>("#dynamic")!;
      for (const [name, entry] of Object.entries(value.style ?? {})) {
        target.style.setProperty(name, String(entry));
      }
    }, props.dynamicallyKeyed);
  }

  async function dispose() {
    await browser?.close();
  }

  return {
    get page() {
      return page;
    },
    initialize,
    initializePage,
    dispose,
  };
}

export type ViewTransitionApplication = ReturnType<typeof createApplication>;

async function prepare() {
  const coreRoot = dirname(fileURLToPath(import.meta.resolve("stillwind")));

  const source = `import {style} from "stillwind/react";
const item = style.viewTransition({ group: "duration-100 ease-linear", old: "opacity-0", new: "opacity-100" });
const aside = style.viewTransition({ old: "opacity-100" });
export const staticallyKeyed = style("block", item.for("hero"));
export const dynamicallyKeyed = style("block", item.for(key));
export const unioned = style("block", item.for("one"), aside.for("two"));
export const guarded = style("block", cond && item.for("guarded"));
export const guardedTogether = style(
  "block",
  cond && item.for("guarded"),
  other && aside.for("other"),
  item.for("base"),
);
export const unnamed = style("block", item);`;

  function expand(css: string, directory = coreRoot): string {
    return css.replace(
      /@import\s+(["'])([^"']+)\1([^;]*);/g,
      (rule, _quote, name: string, tail) => {
        const path =
          name === "stillwind"
            ? resolve(coreRoot, "index.css")
            : name.startsWith("stillwind/")
              ? resolve(coreRoot, "css", name.slice("stillwind/".length))
              : name.startsWith("./")
                ? resolve(directory, name)
                : null;
        if (!path) {
          return rule;
        }
        const body = expand(readFileSync(path, "utf8"), dirname(path));
        const layer = /^\s*layer\(([^)]+)\)\s*$/.exec(tail);
        if (layer) {
          return `@layer ${layer[1]}{${body}}`;
        }
        return tail.trim() === "" ? body : rule;
      },
    );
  }

  const theme = `${expand(readFileSync(resolve(coreRoot, "index.css"), "utf8"))}
@theme { --duration-100: 100ms; --ease-linear: linear; }`;
  const output = compileSource(
    {
      source,
      moduleId: "view-transition.tsx",
      uri: "untitled:view-transition.tsx",
      sourceMapSource: "untitled:view-transition.tsx",
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

  const sandbox = {
    ...runtime,
    key: "item 1",
    cond: true,
    other: true,
  } as typeof runtime & {
    key: string;
    cond: boolean;
    other: boolean;
  } & Record<
      | "staticallyKeyed"
      | "dynamicallyKeyed"
      | "unioned"
      | "guarded"
      | "guardedTogether"
      | "unnamed",
      CompiledProps
    >;
  const moduleCode = output
    .code!.replace(/^import .*?;$/m, "")
    .replaceAll("export const ", "globalThis.");
  runInNewContext(moduleCode, sandbox);

  const props = sandbox;
  expect(props.staticallyKeyed.className).toMatch(/sw_/);
  expect(props.dynamicallyKeyed.style!["view-transition-name"]).toBeTruthy();
  expect(props.unnamed.style).toBe(undefined);

  return { output, props };
}
