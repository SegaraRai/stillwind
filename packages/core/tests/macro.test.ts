import { expect, test } from "vite-plus/test";

for (const framework of ["react", "preact", "solid", "vue", "svelte", "astro"]) {
  test(`${framework} reports missing integration for every uncompiled macro`, async () => {
    const { style } = await import(`stillwind/${framework}`);
    for (const call of [
      () => style("flex"),
      () => style.ref(),
      () => style.keyframes({ from: "opacity-0" }),
      () => style.viewTransition({ old: "opacity-0" }),
    ]) {
      expect(call).toThrow(
        /Stillwind style(?:\.(?:ref|keyframes|viewTransition))?\(\) is a compile-time macro/,
      );
    }
  });
}
