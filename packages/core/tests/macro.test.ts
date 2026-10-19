import type { StyleMacro } from "stillwind/macro";
import { expect, test } from "vite-plus/test";

const frameworks = [
  ["react", () => import("stillwind/react")],
  ["preact", () => import("stillwind/preact")],
  ["solid", () => import("stillwind/solid")],
  ["vue", () => import("stillwind/vue")],
  ["svelte", () => import("stillwind/svelte")],
  ["astro", () => import("stillwind/astro")],
] as const;

for (const [framework, load] of frameworks) {
  test(`${framework} reports missing integration for every uncompiled macro`, async () => {
    const { style: frameworkStyle } = await load();
    const style: StyleMacro = frameworkStyle;
    for (const call of [
      () => style("flex"),
      () => style.ref(),
      () => style.ref("trigger"),
      () => style.anchorScope("none"),
      () => style.keyframes({ from: "opacity-0" }),
      () => style.viewTransition({ old: "opacity-0" }),
      () => style.media("sm"),
      () => style.css("host:block"),
    ]) {
      expect(call).toThrow(
        /Stillwind style(?:\.(?:ref|anchorScope|keyframes|viewTransition|media|css))?\(\) is a compile-time macro/,
      );
    }
  }, 30_000);
}
