import { compileSource } from "@stillwind/native";
import { afterEach, describe, expect, test } from "vite-plus/test";

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

async function compile(expression: string) {
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
  const moduleUrl = URL.createObjectURL(new Blob([output.code!], { type: "text/javascript" }));
  try {
    const module = await import(/* @vite-ignore */ moduleUrl);
    return { css: output.css, props: module.props as Record<string, { className: string }> };
  } finally {
    URL.revokeObjectURL(moduleUrl);
  }
}

let frame: HTMLIFrameElement | undefined;

function documentUnderTest() {
  return frame!.contentDocument!;
}

function element(selector: string) {
  return documentUnderTest().querySelector<HTMLElement>(selector)!;
}

async function expectCss(selector: string, values: Record<string, string>) {
  for (const [name, value] of Object.entries(values)) {
    await expect
      .poll(() => frame!.contentWindow!.getComputedStyle(element(selector)).getPropertyValue(name))
      .toBe(value);
  }
}

async function setDocument(expression: string, html: string) {
  const { css, props } = await compile(expression);
  frame?.remove();
  frame = document.createElement("iframe");
  const ready = new Promise<void>((resolve) => {
    frame!.onload = () => resolve();
  });
  frame.srcdoc = `<style>body {color:black;} ${css}</style>${html.replaceAll(
    /\{(parent|outer|relative|container|target)\}/g,
    (_, name: string) => props[name].className,
  )}`;
  document.body.append(frame);
  await ready;
}

afterEach(() => {
  frame?.remove();
  frame = undefined;
});

describe("ref-scopes", () => {
  test("scope order preserves states and ancestors outside the scope", async () => {
    for (const scopes of ['.in(parent("active")).in(outer)', '.in(outer).in(parent("active"))']) {
      await setDocument(
        `{[relative("outside:on")${scopes}]: 'accent'}`,
        `<main class="outside {outer}"><section class="{parent}" data-active>
          <button id="ref" class="{relative}" data-on></button>
          <span id="target" class="{target}">target</span>
        </section></main><span id="outside" class="{target}">outside</span>`,
      );
      await expectCss("#target", { color: "rgb(255, 0, 0)" });
      await expectCss("#outside", { color: "rgb(0, 0, 0)" });
      element("section").removeAttribute("data-active");
      await expectCss("#target", { color: "rgb(0, 0, 0)" });
      element("section").setAttribute("data-active", "");
      element("#ref").removeAttribute("data-on");
      await expectCss("#target", { color: "rgb(0, 0, 0)" });
    }
  });

  test("ref and target must share the same scope instance", async () => {
    await setDocument(
      "{[relative('on').in(parent)]: 'accent'}",
      `<section class="{parent}"><span id="target" class="{target}">target</span></section>
       <section class="{parent}"><button id="ref" class="{relative}" data-on></button></section>`,
    );
    await expectCss("#target", { color: "rgb(0, 0, 0)" });
    element("#ref").parentElement!.append(element("#target"));
    await expectCss("#target", { color: "rgb(255, 0, 0)" });
  });

  test("container queries test the final target in either wrapper order", async () => {
    for (const expression of [
      "{[container.at('width > 200px')]: {[relative('on').in(parent)]: 'accent'}}",
      "{[relative('on').in(parent)]: {[container.at('width > 200px')]: 'accent'}}",
    ]) {
      await setDocument(
        expression,
        `<section class="{parent}">
          <div class="{container}" style="width:400px"><button class="{relative}" data-on></button></div>
          <div id="container" class="{container}" style="width:100px"><span id="target" class="{target}">target</span></div>
        </section>`,
      );
      await expectCss("#target", { color: "rgb(0, 0, 0)" });
      element("#container").style.width = "300px";
      await expectCss("#target", { color: "rgb(255, 0, 0)" });
    }
  });

  test("ordinary target transforms apply before shared scope conditions", async () => {
    for (const expression of [
      "{'children:': {[relative('on').in(parent)]: 'accent'}}",
      "{[relative('on').in(parent)]: 'children:accent'}",
    ]) {
      await setDocument(
        expression,
        `<main class="{target}"><section class="{parent}">
          <button class="{relative}" data-on></button><span id="target">target</span>
        </section></main>`,
      );
      await expectCss("#target", { color: "rgb(0, 0, 0)" });
      const section = element("section");
      section.classList.add(section.parentElement!.className);
      await expectCss("#target", { color: "rgb(255, 0, 0)" });
    }
  });

  test("scope conditions test the originating element of a target pseudo-element", async () => {
    for (const pseudo of ["before", "after"]) {
      await setDocument(
        `{[relative('on').in(parent)]: '${pseudo}:pseudo'}`,
        `<section class="{parent}"><button id="ref" class="{relative}" data-on></button>
          <span id="target" class="{target}">target</span>
        </section><span id="outside" class="{target}">outside</span>`,
      );
      const color = (id: string) =>
        frame!.contentWindow!.getComputedStyle(element(id), `::${pseudo}`).color;
      await expect.poll(() => color("#target")).toBe("rgb(255, 0, 0)");
      await expect.poll(() => color("#outside")).toBe("rgb(0, 0, 0)");
      element("#ref").removeAttribute("data-on");
      await expect.poll(() => color("#target")).toBe("rgb(0, 0, 0)");
    }
  });
});
