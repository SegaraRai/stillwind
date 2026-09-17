import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  stillwindClassKey,
  stillwindEscapedIdentifier,
  stillwindToFragment,
  stillwindMerge,
  stillwindViewTransitionKey,
  stillwindViewTransitionName,
} from "../src/runtime/react.ts";

void test("composition joins base classes, preserves delta props, and leaves inputs untouched", () => {
  const first = Object.freeze({ className: "a" });
  const second = Object.freeze({ className: "b" });
  const delta = Object.freeze({ [stillwindClassKey]: "own", "data-state": "ready" });
  assert.deepEqual(stillwindMerge(stillwindToFragment(first), stillwindToFragment(second), delta), {
    className: "a b own",
    "data-state": "ready",
  });
  assert.deepEqual(stillwindMerge(stillwindToFragment(first), stillwindToFragment(second)), {
    className: "a b",
  });
  assert.deepEqual(stillwindMerge(), {});
  assert.deepEqual(Object.getOwnPropertySymbols(stillwindMerge(stillwindToFragment(first))), []);
});

void test("view transition names pair a class token with a key", () => {
  assert.equal(stillwindViewTransitionName("sw-vt-card"), "");
  assert.equal(stillwindViewTransitionName("sw-vt-card", "hero"), "sw-vt-card-hero");
  assert.equal(stillwindViewTransitionName("sw-vt-card", "item 1"), "sw-vt-card-item\\ 1");
  assert.equal(stillwindViewTransitionName({ v: "sw-vt-card", w: 7 }), "sw-vt-card-7");
  assert.equal(stillwindViewTransitionName({ v: "sw-vt-card", w: 7 }, "hero"), "sw-vt-card-hero");
  assert.deepEqual(stillwindViewTransitionKey({ v: "sw-vt-card" }, "hero"), {
    v: "sw-vt-card",
    w: "hero",
  });
  assert.throws(() => stillwindViewTransitionName({}), TypeError);
});

void test("escapes identifiers the way the compiler does", () => {
  assert.equal(stillwindEscapedIdentifier("hero"), "hero");
  assert.equal(stillwindEscapedIdentifier("1hero"), "\\31 hero");
  assert.equal(stillwindEscapedIdentifier("-1hero"), "-\\31 hero");
  assert.equal(stillwindEscapedIdentifier("a b"), "a\\ b");
  assert.equal(stillwindEscapedIdentifier(""), "");
});

void test("composes names matching the shared escape vectors", () => {
  const fixtures = JSON.parse(
    readFileSync(
      new URL("../../../fixtures/view-transitions/name-escapes.json", import.meta.url),
      "utf8",
    ),
  ) as { readonly token: string; readonly vectors: ReadonlyArray<{ key: string; name: string }> };
  assert.notEqual(fixtures.vectors.length, 0);
  for (const vector of fixtures.vectors) {
    assert.equal(
      stillwindViewTransitionName(fixtures.token, vector.key),
      vector.name,
      JSON.stringify(vector.key),
    );
  }
});
