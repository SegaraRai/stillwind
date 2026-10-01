import { readFileSync } from "node:fs";

import { expect, test } from "vite-plus/test";

import { stillwindEscapedIdentifier, stillwindViewTransitionName } from "./shared.ts";

test("view transition names pair a class token with a key", () => {
  expect(stillwindViewTransitionName("sw-vt-card", "hero")).toBe("sw-vt-card-hero");
  expect(stillwindViewTransitionName("sw-vt-card", "item 1")).toBe("sw-vt-card-item\\ 1");
  expect(stillwindViewTransitionName({ v: "sw-vt-card" }, 7)).toBe("sw-vt-card-7");
  expect(stillwindViewTransitionName({ v: "sw-vt-card", w: 7 }, "hero")).toBe("sw-vt-card-hero");
  expect(() => stillwindViewTransitionName({}, "hero")).toThrow(TypeError);
});

test("escapes identifiers the way the compiler does", () => {
  expect(stillwindEscapedIdentifier("hero")).toBe("hero");
  expect(stillwindEscapedIdentifier("1hero")).toBe("\\31 hero");
  expect(stillwindEscapedIdentifier("-1hero")).toBe("-\\31 hero");
  expect(stillwindEscapedIdentifier("a b")).toBe("a\\ b");
  expect(stillwindEscapedIdentifier("")).toBe("");
});

test("composes names matching the shared escape vectors", () => {
  const fixtures = JSON.parse(
    readFileSync(
      new URL("../../../../fixtures/view-transitions/name-escapes.json", import.meta.url),
      "utf8",
    ),
  ) as { readonly token: string; readonly vectors: ReadonlyArray<{ key: string; name: string }> };
  expect(fixtures.vectors.length).not.toBe(0);
  for (const vector of fixtures.vectors) {
    expect(
      stillwindViewTransitionName(fixtures.token, vector.key),
      JSON.stringify(vector.key),
    ).toBe(vector.name);
  }
});
