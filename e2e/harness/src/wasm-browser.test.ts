import { __napiBindingTarget, compileSource, minifyCss } from "@stillwind/native";
import { expect, test } from "vite-plus/test";

test("the root browser entry compiles and minifies CSS with WASI", () => {
  expect(crossOriginIsolated).toBe(true);
  expect(__napiBindingTarget).toBe("wasm32-wasi");
  const output = compileSource({
    source: "import {style} from 'stillwind/react'; export const props=style('flex');",
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
  expect(() => minifyCss(".a { color red }")).toThrow(/CSS parse failed/);
});
