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
