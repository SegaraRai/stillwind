import { BrowserProject } from "@stillwind/host/browser";
import { __napiBindingTarget, fingerprint, compileSource, minifyCss } from "@stillwind/native";
import { expect, test } from "vite-plus/test";

test("the root browser entry compiles and minifies CSS with WASI", () => {
  expect(crossOriginIsolated).toBe(true);
  expect(__napiBindingTarget).toBe("wasm32-wasi");
  const source =
    '/* 日本語 🧭 */\r\nimport {style} from "stillwind/react";export const props=style("flex missing");';
  const output = compileSource({
    source,
    moduleId: "browser.tsx",
    uri: "file:///browser.tsx",
    sourceMapSource: "file:///browser.tsx",
    language: "tsx",
    moduleKind: "unambiguous",
    css: {
      content: "@utility flex{display:flex}@stillwind-slot utilities;",
      name: "root.css",
      uri: "file:///root.css",
      sourceMapSource: "file:///root.css",
    },
  });
  expect(output.code).toContain("className");
  expect(minifyCss(output.css).css).toContain("display:flex");
  expect(output.diagnostics).toStrictEqual([]);
  expect(output.reports[0].primary.location.revision).toBe(fingerprint(source));
  expect(fingerprint(new TextEncoder().encode(source))).toBe(fingerprint(source));
  expect(() => minifyCss(".a { color red }")).toThrow(/CSS parse failed/);
});

test("browser CSS colors, picker edits, and statement sorting use WASI queries", async () => {
  const id = "/workspace/theme.css";
  const source =
    "@stillwind-theme-namespace --color-* [color]; @theme { --color-brand:red; } @utility a {display:block;} @utility b {padding:1px;} @utility bg-* {background-color:--value(--color-*);}\n/* 日本語 🦀 */ .a { @apply b; @apply a; @apply bg-brand; color:var(--color-brand); }";
  const project = new BrowserProject({
    root: "/workspace",
    files: new Map([[id, source]]),
    config: {
      css: "theme.css",
      files: ["*.tsx"],
      typeFacts: false,
      lint: false,
      sort: { applyStatements: true },
    },
  });
  try {
    const colors = (await project.query(id, { kind: "colors" }))!.colors!;
    expect(colors.map((color) => source.slice(color.start, color.end))).toEqual([
      "bg-brand",
      "var(--color-brand)",
    ]);
    const picker = await project.query(id, {
      kind: "colorPresentation",
      ...colors[0],
      color: { red: 0, green: 0, blue: 1, alpha: 1 },
    });
    expect(picker!.edits![0].newText).toBe("bg-[#0000ff]");
    const reference = await project.query(id, {
      kind: "colorPresentation",
      ...colors[1],
      color: { red: 0, green: 1, blue: 0, alpha: 1 },
    });
    expect(reference!.edits![0].newText).toBe("#00ff00");
    const sorted = (await project.query(id, { kind: "sort" }))!.edits!;
    expect(sorted).toHaveLength(1);
    expect(sorted[0].newText).toBe("@apply a; @apply b; @apply bg-brand;");
    const offset = source.indexOf("@utility") + 1;
    const ordinary = (await project.query(id, {
      kind: "completion",
      offset,
    }))!.completions!;
    expect(ordinary.some((item) => item.label === "@stillwind-utility")).toBe(false);
    const registry = (await project.query(id, {
      kind: "completion",
      offset,
      registryDirectives: true,
    }))!.completions!;
    expect(registry).toContainEqual(
      expect.objectContaining({
        label: "@stillwind-utility",
        detail: "Registry authoring · Stillwind syntax",
      }),
    );
  } finally {
    await project.close();
  }
});

test("browser editor sorting groups inputs and orders utility and attribute names independently", async () => {
  const source = `import {style} from "stillwind/react";
// 日本語 🦀
const base=style({"hover:":"flex"}); const ref=style.ref();
const value=style(["flex block",base,ref.asContainer(),ref,{"data-z":1,"aria-label":"x","data-a":2},{"--x":1}]);`;
  const make = (code: string) =>
    new BrowserProject({
      root: "/workspace",
      files: new Map([
        ["/workspace/entry.tsx", code],
        [
          "/workspace/theme.css",
          "@utility block{display:block} @utility flex{display:flex} @variant hover{&:hover{@slot;}} @stillwind-slot utilities;",
        ],
      ]),
      config: {
        css: "theme.css",
        files: ["*.tsx"],
        typeFacts: false,
        sort: {
          attributes: true,
        },
      },
    });
  const project = make(source);
  try {
    const result = (await project.query("/workspace/entry.tsx", {
      kind: "sort",
    }))!;
    let sorted = source;
    for (const edit of [...result.edits!].reverse()) {
      sorted = sorted.slice(0, edit.start) + edit.newText + sorted.slice(edit.end);
    }
    expect(sorted).toContain(
      'style([ref,ref.asContainer(),base,"block flex",{"--x":1},{"aria-label":"x","data-a":2,"data-z":1}])',
    );
    expect(
      (await project.query("/workspace/entry.tsx", {
        kind: "sort",
      }))!.edits,
    ).toEqual(result.edits);
    const next = make(sorted);
    try {
      expect(
        (await next.query("/workspace/entry.tsx", {
          kind: "sort",
        }))!.edits,
      ).toEqual([]);
    } finally {
      await next.close();
    }
  } finally {
    await project.close();
  }
});
