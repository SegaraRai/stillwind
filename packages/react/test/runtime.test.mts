import assert from "node:assert/strict";
import test from "node:test";
import {
  stillwindAnimation,
  stillwindAssetClass,
  stillwindHandleClass,
  stillwindRef,
  stillwindScope,
  stillwindViewTransition,
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

void test("selects classes carried by dynamic animation and view transition values", () => {
  const fade = stillwindAnimation("--sw-a-fade");
  const title = stillwindViewTransition("--sw-v-title");

  assert.equal(stillwindAssetClass(fade("infinite"), { "--sw-a-fade": "a_fade" }), "a_fade");
  assert.equal(stillwindAssetClass(title, { "--sw-v-title": "v_title" }), "v_title");
  assert.throws(() => stillwindAssetClass({ identity: "user-owned" }, {}), TypeError);
});
