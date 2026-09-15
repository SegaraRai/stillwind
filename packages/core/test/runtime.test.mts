import assert from "node:assert/strict";
import test from "node:test";

import {
  stillwindAnimation,
  stillwindAssetClass,
  stillwindClassKey,
  stillwindToFragment,
  stillwindMerge,
  stillwindViewTransition,
  stillwindViewTransitionStyle,
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

void test("view-transition styles share properties and encode keyed instance names", () => {
  const card = stillwindViewTransition("--sw-v-card");
  const candidates = ["--sw-v-card"];
  assert.deepEqual(
    stillwindViewTransitionStyle(card, undefined, candidates, "--sw-vt-name", "--sw-vt-class"),
    { "--sw-vt-name": "--sw-v-card", "--sw-vt-class": "--sw-v-card" },
  );
  assert.deepEqual(
    stillwindViewTransitionStyle(card, "item 1", candidates, "--sw-vt-name", "--sw-vt-class"),
    { "--sw-vt-name": "--sw-v-card-k-69-74-65-6d-20-31", "--sw-vt-class": "--sw-v-card" },
  );
  assert.deepEqual(
    stillwindViewTransitionStyle(
      card.for("item 1"),
      undefined,
      candidates,
      "--sw-vt-name",
      "--sw-vt-class",
    ),
    { "--sw-vt-name": "--sw-v-card-k-69-74-65-6d-20-31", "--sw-vt-class": "--sw-v-card" },
  );
});

void test("selects classes carried by dynamic animation and view transition values", () => {
  const fade = stillwindAnimation("--sw-a-fade");
  const title = stillwindViewTransition("--sw-v-title");

  assert.equal(stillwindAssetClass(fade("infinite"), { "--sw-a-fade": "a_fade" }), "a_fade");
  assert.equal(stillwindAssetClass(title, { "--sw-v-title": "v_title" }), "v_title");
  assert.throws(() => stillwindAssetClass({ identity: "user-owned" }, {}), TypeError);
});
