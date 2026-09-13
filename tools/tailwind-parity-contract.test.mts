import assert from "node:assert/strict";
import { test } from "node:test";

import {
  comparisonCss,
  parityFixture,
  fixtureThemeKeyframes,
} from "./lib/tailwind-parity-contract.ts";

await test("reserved layers and empty custom-property whitespace are explicit equivalences", () => {
  assert.equal(
    comparisonCss("@layer utilities{.x{--empty: ;display:flex}}"),
    comparisonCss("@layer theme;@layer utilities{.x{--empty:;display:flex}}"),
  );
});

await test("layer bodies, registration semantics, fallbacks and interfering order stay strict", () => {
  for (const [a, b] of [
    ["@layer theme{.x{color:red}}", ".x{color:red}"],
    [".x{width:var(--x)}", ".x{width:var(--x,0px)}"],
    [".x{--x:1}", '@property --x{syntax:"*";inherits:false}.x{--x:1}'],
    [".x{opacity:.5;opacity:.7}", ".x{opacity:.7;opacity:.5}"],
    ["@layer other;@layer theme{.x{color:red}}", "@layer theme{.x{color:red}}"],
  ])
    assert.notEqual(comparisonCss(a), comparisonCss(b));
});

await test("only upstream theme-owned keyframes adopt the preset slot", () => {
  const frame = "@keyframes spin{to{opacity:0}}";
  const frames = new Set(["spin"]);
  assert.equal(comparisonCss(frame, frames), comparisonCss(`@layer theme{${frame}}`));
  assert.notEqual(comparisonCss(frame, frames), comparisonCss(frame));
  assert.notEqual(
    comparisonCss(frame, frames),
    comparisonCss(`@media print{@layer theme{${frame}}}`),
  );
  assert.notEqual(comparisonCss(frame, new Set()), comparisonCss(`@layer theme{${frame}}`));
});

await test("prefix migration is explicit and source(none) leaves manual candidates available", () => {
  const fixture = parityFixture('@import "tailwindcss" prefix(tw) source(none);');
  assert.equal(fixture.css, '@import "tailwindcss";');
  assert.equal(fixture.candidate("tw:hover:flex"), "hover:flex");
  assert.throws(() => fixture.candidate("flex"));
  assert.throws(() =>
    parityFixture('@import "tailwindcss" prefix(a);@import "tailwindcss" prefix(b);'),
  );
});

await test("malformed CSS must not be recovered into an accepted output", () => {
  assert.throws(() => comparisonCss(".x{color:var(}"));
});

await test("theme grammar includes the first keyframes and excludes authored collisions", () => {
  assert.deepEqual(
    [...fixtureThemeKeyframes("@theme{--animate-x:x 1s;@keyframes x{to{opacity:0}}}")],
    ["x"],
  );
  assert.ok(fixtureThemeKeyframes('@import "tailwindcss/theme";').has("spin"));
  assert.ok(!fixtureThemeKeyframes('@import "tailwindcss/utilities";').has("spin"));
  assert.ok(
    !fixtureThemeKeyframes('@import "tailwindcss/theme";@keyframes spin{to{opacity:0}}').has(
      "spin",
    ),
  );
});
