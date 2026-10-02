import { runInNewContext } from "node:vm";

import { expectCss } from "@stillwind/e2e-harness/browser";
import { registerScenario } from "@stillwind/e2e-harness/scenario";
import { compileSource } from "@stillwind/native";
import { chromium, type Browser, type Page } from "playwright";
import { afterAll, beforeAll, test, expect } from "vite-plus/test";

const theme = `
@variant on { &[data-on] { @slot; } }
@variant active { &[data-active] { @slot; } }
@variant outside { .outside & { @slot; } }
@variant children { & > * { @slot; } }
@variant before { &::before { @slot; } }
@variant after { &::after { @slot; } }
@utility accent { color:red; }
@utility pseudo { content:"ok"; color:red; }
@stillwind-slot utilities;`;

function compile(expression: string) {
  const output = compileSource({
    source: `import {style} from 'stillwind/react';
const parent = style.ref('parent');
const outer = style.ref('outer');
const relative = style.ref('relative');
const container = style.ref('container');
export const props = {
  parent: style(parent), outer: style(outer), relative: style(relative),
  container: style(container, container.asContainer('inline-size')),
  target: style(${expression}),
};`,
    moduleId: "scopes.ts",
    uri: "untitled:scopes.ts",
    sourceMapSource: "untitled:scopes.ts",
    language: "ts",
    moduleKind: "unambiguous",
    css: {
      content: theme,
      name: "root.css",
      uri: "untitled:root.css",
      sourceMapSource: "untitled:root.css",
    },
  });
  expect(output.diagnostics).toEqual([]);
  expect(output.code).toBeTruthy();
  const sandbox = {} as { props: Record<string, { className: string }> };
  runInNewContext(output.code!.replace("export const props", "globalThis.props"), sandbox);
  return { css: output.css, props: sandbox.props };
}

async function setDocument(page: Page, expression: string, html: string) {
  const { css, props } = compile(expression);
  await page.setContent(
    `<style>body {color:black;} ${css}</style>${html.replaceAll(
      /\{(parent|outer|relative|container|target)\}/g,
      (_, name: string) => props[name].className,
    )}`,
  );
}

registerScenario("ref-scopes", new URL("../", import.meta.url), () => {
  let browser: Browser;
  let page: Page;
  beforeAll(async () => {
    browser = await chromium.launch({ channel: "chrome", headless: true });
    page = await browser.newPage();
  });
  afterAll(async () => browser?.close());

  test("scope order preserves states and ancestors outside the scope", async () => {
    for (const scopes of ['.in(parent("active")).in(outer)', '.in(outer).in(parent("active"))']) {
      await setDocument(
        page,
        `{[relative("outside:on")${scopes}]: 'accent'}`,
        `<main class="outside {outer}"><section class="{parent}" data-active>
          <button id="ref" class="{relative}" data-on></button>
          <span id="target" class="{target}">target</span>
        </section></main><span id="outside" class="{target}">outside</span>`,
      );
      await expectCss(page, "#target", { color: "rgb(255, 0, 0)" });
      await expectCss(page, "#outside", { color: "rgb(0, 0, 0)" });
      await page.locator("section").evaluate((element) => element.removeAttribute("data-active"));
      await expectCss(page, "#target", { color: "rgb(0, 0, 0)" });
      await page.locator("section").evaluate((element) => element.setAttribute("data-active", ""));
      await page.locator("#ref").evaluate((element) => element.removeAttribute("data-on"));
      await expectCss(page, "#target", { color: "rgb(0, 0, 0)" });
    }
  });

  test("ref and target must share the same scope instance", async () => {
    await setDocument(
      page,
      "{[relative('on').in(parent)]: 'accent'}",
      `<section class="{parent}"><span id="target" class="{target}">target</span></section>
       <section class="{parent}"><button id="ref" class="{relative}" data-on></button></section>`,
    );
    await expectCss(page, "#target", { color: "rgb(0, 0, 0)" });
    await page.locator("#target").evaluate((target) => {
      document.querySelector("#ref")!.parentElement!.append(target);
    });
    await expectCss(page, "#target", { color: "rgb(255, 0, 0)" });
  });

  test("container queries test the final target in either wrapper order", async () => {
    for (const expression of [
      "{[container.at('width > 200px')]: {[relative('on').in(parent)]: 'accent'}}",
      "{[relative('on').in(parent)]: {[container.at('width > 200px')]: 'accent'}}",
    ]) {
      await setDocument(
        page,
        expression,
        `<section class="{parent}">
          <div class="{container}" style="width:400px"><button class="{relative}" data-on></button></div>
          <div id="container" class="{container}" style="width:100px"><span id="target" class="{target}">target</span></div>
        </section>`,
      );
      await expectCss(page, "#target", { color: "rgb(0, 0, 0)" });
      await page.locator("#container").evaluate((element: HTMLElement) => {
        element.style.width = "300px";
      });
      await expectCss(page, "#target", { color: "rgb(255, 0, 0)" });
    }
  });

  test("ordinary target transforms apply before shared scope conditions", async () => {
    for (const expression of [
      "{'children:': {[relative('on').in(parent)]: 'accent'}}",
      "{[relative('on').in(parent)]: 'children:accent'}",
    ]) {
      await setDocument(
        page,
        expression,
        `<main class="{target}"><section class="{parent}">
          <button class="{relative}" data-on></button><span id="target">target</span>
        </section></main>`,
      );
      await expectCss(page, "#target", { color: "rgb(0, 0, 0)" });
      await page.locator("section").evaluate((element) => {
        element.classList.add(element.parentElement!.className);
      });
      await expectCss(page, "#target", { color: "rgb(255, 0, 0)" });
    }
  });

  test("scope conditions test the originating element of a target pseudo-element", async () => {
    for (const pseudo of ["before", "after"]) {
      await setDocument(
        page,
        `{[relative('on').in(parent)]: '${pseudo}:pseudo'}`,
        `<section class="{parent}"><button id="ref" class="{relative}" data-on></button>
          <span id="target" class="{target}">target</span>
        </section><span id="outside" class="{target}">outside</span>`,
      );
      const color = (id: string) =>
        page
          .locator(id)
          .evaluate((element, pseudo) => getComputedStyle(element, pseudo).color, `::${pseudo}`);
      await expect.poll(() => color("#target")).toBe("rgb(255, 0, 0)");
      await expect.poll(() => color("#outside")).toBe("rgb(0, 0, 0)");
      await page.locator("#ref").evaluate((element) => element.removeAttribute("data-on"));
      await expect.poll(() => color("#target")).toBe("rgb(0, 0, 0)");
    }
  });
});
