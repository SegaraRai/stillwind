/// <reference types="vite-plus/client" />

import { compileSource } from "@stillwind/native";
import { afterEach, expect, test } from "vite-plus/test";

import templates from "stillwind/templates.css?raw";

const theme = `
@theme { --color-brand: rgb(1, 2, 3); }
@stillwind-property --shift { syntax: "<length>"; inherits: false; initial-value: 0px; }
@utility shift { --shift: 20px; }
@utility moved { translate: var(--shift) 0px; }
@utility bg-* { background-color: --value(--color-*); }
:root, :host { @stillwind-slot theme; }
@stillwind-slot utilities;
${templates}`;

/**
 * Compiles one `style.css()` call with the built-in templates and returns its string.
 *
 * @param inputs - Source text of the call's arguments.
 * @returns The stylesheet text that the call lowers to.
 */
async function sheet(inputs: string): Promise<string> {
  const output = compileSource({
    source: `import {style} from 'stillwind/react'; export const sheet = style.css(${inputs});`,
    moduleId: "sheet.ts",
    uri: "untitled:sheet.ts",
    sourceMapSource: "untitled:sheet.ts",
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
  const moduleUrl = URL.createObjectURL(new Blob([output.code!], { type: "text/javascript" }));
  try {
    return ((await import(/* @vite-ignore */ moduleUrl)) as { sheet: string }).sheet;
  } finally {
    URL.revokeObjectURL(moduleUrl);
  }
}

let host: HTMLElement | undefined;

afterEach(() => {
  host?.remove();
  host = undefined;
});

test("the default template initializes properties and theme values inside a shadow root", async () => {
  const css = await sheet(`"[.parent]:shift [.parent]:moved [.child]:moved [.child]:bg-brand"`);
  expect(css).not.toContain("@property");
  host = document.createElement("div");
  document.body.append(host);
  const root = host.attachShadow({ mode: "open" });
  root.innerHTML = `<div class="parent"><div class="child"></div></div>`;
  const stylesheet = new CSSStyleSheet();
  stylesheet.replaceSync(css);
  root.adoptedStyleSheets = [stylesheet];
  const style = (selector: string) => getComputedStyle(root.querySelector(selector)!);
  expect(style(".parent").translate).toBe("20px");
  // A registration inside a shadow root has no effect, so only the initial value on
  // every element keeps the parent's value from reaching the child.
  expect(style(".child").translate).toBe("0px");
  expect(style(".child").backgroundColor).toBe("rgb(1, 2, 3)");
});

test("the selector option styles the shadow host", async () => {
  const css = await sheet(`"bg-brand", { selector: ":host" }`);
  host = document.createElement("div");
  document.body.append(host);
  const root = host.attachShadow({ mode: "open" });
  const stylesheet = new CSSStyleSheet();
  stylesheet.replaceSync(css);
  root.adoptedStyleSheets = [stylesheet];
  expect(getComputedStyle(host).backgroundColor).toBe("rgb(1, 2, 3)");
});
