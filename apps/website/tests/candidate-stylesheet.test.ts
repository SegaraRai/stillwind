import { minifyCss, NativeDesignSystemLoader, NativeProjectSession } from "@stillwind/native";
import { expect, test } from "vite-plus/test";

import { recordDefaultStylesheet } from "../src/lib/docs/candidates/record";
import { replayStylesheet } from "../src/lib/docs/candidates/stylesheet";
import { referenceCompiler } from "../src/lib/docs/reference/compiler";

test("the optimized candidate lookup stylesheet compiles the same document as the source package", async () => {
  const original = await referenceCompiler();
  const loader = new NativeDesignSystemLoader({});
  replayStylesheet(loader, await recordDefaultStylesheet());
  const candidates = [
    "p-4",
    "space-x-2",
    "bg-blue-500/50",
    "hover:underline",
    "md:flex",
    "group-hover:opacity-50",
    "before:content-['a__b']",
    "-translate-x-1/2",
    "animate-spin",
    "[padding:16px]",
  ];
  const source = `import { style } from "stillwind/react"; export const props = style(${JSON.stringify(candidates.join(" "))});`;
  const expected = original.document(source);
  const session = new NativeProjectSession(loader, false);
  const [actual] = session.compile([
    {
      kind: "direct",
      moduleId: "Reference.tsx",
      document: { uri: "file:///Reference.tsx", format: "jsx", code: source },
      syntax: { language: "tsx", moduleKind: "module" },
    },
  ]).modules;
  expect(actual!.diagnostics).toEqual([]);
  expect(actual!.code).toBe(expected.module!.code);
  expect(minifyCss(session.css().css).css).toBe(minifyCss(expected.css).css);
});
