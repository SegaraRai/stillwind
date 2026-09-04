import assert from "node:assert/strict";
import test from "node:test";
import {
  stillwindAnimation,
  stillwindAssetClass,
  stillwindHandleClass,
  stillwindRef,
  stillwindScope,
  stillwindViewTransition,
  stillwindViewTransitionStyle,
} from "../src/runtime.ts";

void test("selects the marker carried by a dynamic compiled handle", () => {
  const first = stillwindRef("s_first_01234567");
  const second = stillwindRef("s_second_89abcdef");
  const selected = process.argv.includes("--select-first") ? first : second;

  assert.equal(stillwindHandleClass(selected), "s_second_89abcdef");
  assert.equal(stillwindHandleClass(selected.anchor), "_swp_s_second_89abcdef_anchor");
  assert.equal(
    stillwindHandleClass(selected.container("inline-size")),
    "_swp_s_second_89abcdef_inline-size",
  );
  assert.equal(stillwindHandleClass(stillwindScope("s_menu_76543210")), "s_menu_76543210");
  assert.throws(() => stillwindHandleClass({ className: "user-owned" }), TypeError);
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
