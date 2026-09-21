import assert from "node:assert/strict";
import test from "node:test";

for (const framework of ["react", "preact", "solid", "vue", "svelte", "astro"]) {
  void test(`${framework} reports missing integration for every uncompiled macro`, async () => {
    const { style } = await import(`stillwind/${framework}`);
    for (const call of [
      () => style("flex"),
      () => style.ref(),
      () => style.keyframes({ from: "opacity-0" }),
      () => style.viewTransition({ old: "opacity-0" }),
    ]) {
      assert.throws(call, (error: unknown) => {
        assert.ok(error instanceof Error);
        assert.match(
          error.message,
          /Stillwind style(?:\.(?:ref|keyframes|viewTransition))?\(\) is a compile-time macro/,
        );
        assert.doesNotMatch(error.message, /react|Vite/);
        return true;
      });
    }
  });
}
