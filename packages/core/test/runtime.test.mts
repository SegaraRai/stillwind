import assert from "node:assert/strict";
import test from "node:test";

import {
  stillwindAnimation,
  stillwindAssetClass,
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
  assert.equal(stillwindViewTransitionName("--sw-v-card"), "");
  assert.equal(stillwindViewTransitionName("--sw-v-card", "hero"), "--sw-v-card-hero");
  assert.equal(stillwindViewTransitionName("--sw-v-card", "item 1"), "--sw-v-card-item\\ 1");
  assert.equal(stillwindViewTransitionName({ v: "--sw-v-card", k: 7 }), "--sw-v-card-7");
  assert.equal(stillwindViewTransitionName({ v: "--sw-v-card", k: 7 }, "hero"), "--sw-v-card-hero");
  assert.deepEqual(stillwindViewTransitionKey({ v: "--sw-v-card" }, "hero"), {
    v: "--sw-v-card",
    k: "hero",
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

void test("selects classes carried by dynamic animation and view transition values", () => {
  const fade = stillwindAnimation("--sw-a-fade");

  assert.equal(stillwindAssetClass(fade("infinite"), { "--sw-a-fade": "a_fade" }), "a_fade");
  assert.equal(
    stillwindAssetClass({ v: "--sw-v-title" }, { "--sw-v-title": "v_title" }),
    "v_title",
  );
  assert.throws(() => stillwindAssetClass({ identity: "user-owned" }, {}), TypeError);
});
