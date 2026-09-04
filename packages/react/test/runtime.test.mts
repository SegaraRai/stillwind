import assert from "node:assert/strict";
import test from "node:test";
import { stillwindHandleClass, stillwindRef, stillwindScope } from "../src/runtime.ts";

void test("selects the marker carried by a dynamic compiled handle", () => {
  const first = stillwindRef("s_first_01234567");
  const second = stillwindRef("s_second_89abcdef");
  const selected = process.argv.includes("--select-first") ? first : second;

  assert.equal(stillwindHandleClass(selected), "s_second_89abcdef");
  assert.equal(stillwindHandleClass(stillwindScope("s_menu_76543210")), "s_menu_76543210");
  assert.throws(() => stillwindHandleClass({ className: "user-owned" }), TypeError);
});
