import assert from "node:assert/strict";
import test from "node:test";

void test("loads the current platform package through the public loader", async () => {
  const binding = await import("../index.js");
  assert.equal(typeof binding.compileReact, "function");
});
