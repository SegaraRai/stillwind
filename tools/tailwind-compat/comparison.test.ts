import { expect, test } from "vite-plus/test";

import { comparisonCss, fixtureThemeKeyframes } from "./comparison.ts";

test("reserved layers and empty custom-property whitespace are explicit equivalences", () => {
  expect(comparisonCss("@layer utilities{.x{--empty: ;display:flex}}")).toBe(
    comparisonCss("@layer theme;@layer utilities{.x{--empty:;display:flex}}"),
  );
});

test("background image and shorthand supports queries share the comparison contract", () => {
  for (const condition of [
    "(background-image:linear-gradient(in lab,red,red))",
    "not (background-image:linear-gradient(in lab,red,red))",
    "(display:grid) and (background-image:linear-gradient(in lab,red,red))",
  ]) {
    expect(comparisonCss(`@media screen{@supports ${condition}{.x{color:red}}}`)).toBe(
      comparisonCss(
        `@media screen{@supports ${condition.replace("background-image", "background")}{.x{color:red}}}`,
      ),
    );
  }
});

test("supports query values, bodies and ordinary background declarations stay strict", () => {
  const query = "@supports (background-image:linear-gradient(in lab,red,red))";
  const css = `${query}{.x{color:red}}`;
  for (const other of [
    css.replace("in lab", "in oklab"),
    css.replace("color:red", "color:blue"),
    css.replace("background-image", "mask-image"),
    css.replace("@supports (", "@supports not ("),
  ]) {
    expect(comparisonCss(css)).not.toBe(comparisonCss(other));
  }
  expect(comparisonCss(`${query}{.x{background-image:linear-gradient(red,blue)}}`)).not.toBe(
    comparisonCss(`${query}{.x{background:linear-gradient(red,blue)}}`),
  );
});

test("layer bodies, registration semantics, fallbacks and interfering order stay strict", () => {
  for (const [a, b] of [
    ["@layer theme{.x{color:red}}", ".x{color:red}"],
    [".x{width:var(--x)}", ".x{width:var(--x,0px)}"],
    [".x{--x:1}", '@property --x{syntax:"*";inherits:false}.x{--x:1}'],
    [".x{opacity:.5;opacity:.7}", ".x{opacity:.7;opacity:.5}"],
    ["@layer other;@layer theme{.x{color:red}}", "@layer theme{.x{color:red}}"],
  ]) {
    expect(comparisonCss(a)).not.toBe(comparisonCss(b));
  }
});

test("only upstream theme-owned keyframes adopt the preset slot", () => {
  const frame = "@keyframes spin{to{opacity:0}}";
  const frames = new Set(["spin"]);
  expect(comparisonCss(frame, frames)).toBe(comparisonCss(`@layer theme{${frame}}`));
  expect(comparisonCss(frame, frames)).not.toBe(comparisonCss(frame));
  expect(comparisonCss(frame, frames)).not.toBe(
    comparisonCss(`@media print{@layer theme{${frame}}}`),
  );
  expect(comparisonCss(frame, new Set())).not.toBe(comparisonCss(`@layer theme{${frame}}`));
});

test("malformed CSS must not be recovered into an accepted output", () => {
  expect(() => comparisonCss(".x{color:var(}")).toThrow();
});

test("theme grammar includes the first keyframes and excludes authored collisions", () => {
  expect([
    ...fixtureThemeKeyframes("@theme{--animate-x:x 1s;@keyframes x{to{opacity:0}}}"),
  ]).toStrictEqual(["x"]);
  expect(fixtureThemeKeyframes('@import "tailwindcss/theme";').has("spin")).toBeTruthy();
  expect(!fixtureThemeKeyframes('@import "tailwindcss/utilities";').has("spin")).toBeTruthy();
  expect(
    !fixtureThemeKeyframes('@import "tailwindcss/theme";@keyframes spin{to{opacity:0}}').has(
      "spin",
    ),
  ).toBeTruthy();
});
