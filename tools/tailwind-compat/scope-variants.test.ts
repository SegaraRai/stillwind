import { resolve } from "node:path";

import { compileTailwindCss } from "@stillwind-internal/tools-tailwind/compiler";
import { expect, test } from "vite-plus/test";

import { comparisonCss, compileParity } from "./comparison.ts";
import { stillwindFixtureCss } from "./stillwind-css.ts";

test.each([
  ["(.card) to (.stop)", "@slot;"],
  ["(.card &)", "@slot;"],
  ["(& .card)", "@slot;"],
  ["(&, .card)", "@slot;"],
  ["(.card) to (& .stop)", "@slot;"],
  ["", "@slot;"],
  ["to (.stop)", "@slot;"],
  ["(.outer)", "@scope (.inner) { @slot; }"],
  ["(.outer &)", "@scope (.inner) { @slot; }"],
  ["(.outer)", "@scope (& .inner) { @slot; }"],
  ["(.card)", "@supports (display:flex) { @slot; }"],
  ["(.card)", "&:hover { @slot; }"],
  ["(.card)", "& { color:red; @slot; }"],
  ["(:is(.card,::before))", "@slot;"],
])("scope %s with body %s matches Tailwind", async (prelude, body) => {
  const input = `@import "tailwindcss/utilities";@custom-variant scoped { @scope ${prelude} { ${body} } }`;
  const expected = await compileTailwindCss({
    css: input,
    candidates: ["scoped:flex"],
    base: resolve(import.meta.dirname, "../../fixtures/tailwind-v4"),
  });
  const actual = compileParity(
    'import {style} from "stillwind/react";export const item=style("scoped:flex");',
    stillwindFixtureCss(input),
  );
  expect(actual.diagnostics).toEqual([]);
  expect(actual.classes).toHaveLength(1);
  const css = actual.css.replaceAll(actual.classes[0].className, "scoped\\:flex");
  expect(comparisonCss(css)).toBe(comparisonCss(expected));
});
