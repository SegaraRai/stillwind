import { expect, test } from "vite-plus/test";

import { comparisonCss, parityFixture, fixtureThemeKeyframes } from "./comparison.ts";

test("reserved layers and empty custom-property whitespace are explicit equivalences", () => {
  expect(comparisonCss("@layer utilities{.x{--empty: ;display:flex}}")).toBe(
    comparisonCss("@layer theme;@layer utilities{.x{--empty:;display:flex}}"),
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

test("prefix migration is explicit and source(none) leaves manual candidates available", () => {
  const fixture = parityFixture('@import "tailwindcss" prefix(tw) source(none);');
  expect(fixture.css).toBe('@import "tailwindcss";');
  expect(fixture.candidate("tw:hover:flex")).toBe("hover:flex");
  expect(() => fixture.candidate("flex")).toThrow();
  expect(() =>
    parityFixture('@import "tailwindcss" prefix(a);@import "tailwindcss" prefix(b);'),
  ).toThrow();
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
