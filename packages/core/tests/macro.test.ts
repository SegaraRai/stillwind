import type { StyleMacroWithoutForward } from "stillwind/macro";
import { expect, test } from "vite-plus/test";

const frameworks = [
  ["alpine", () => import("stillwind/alpine")],
  ["astro", () => import("stillwind/astro")],
  ["lit", () => import("stillwind/lit")],
  ["preact", () => import("stillwind/preact")],
  ["qwik", () => import("stillwind/qwik")],
  ["react", () => import("stillwind/react")],
  ["solid", () => import("stillwind/solid")],
  ["svelte", () => import("stillwind/svelte")],
  ["vanilla", () => import("stillwind/vanilla")],
  ["vue", () => import("stillwind/vue")],
] as const;

for (const [framework, load] of frameworks) {
  test(`${framework} reports missing integration for every uncompiled macro`, async () => {
    const { style: frameworkStyle } = await load();
    const style: StyleMacroWithoutForward = frameworkStyle;
    for (const call of [
      () => style("flex"),
      () => style.ref(),
      () => style.ref("trigger"),
      () => style.anchorScope("none"),
      () => style.keyframes({ from: "opacity-0" }),
      () => style.viewTransition({ old: "opacity-0" }),
      () => style.when("hover"),
      () => style.cva("flex"),
      () => style.media("sm"),
      () => style.css("host:block"),
      ...("forward" in frameworkStyle ? [() => frameworkStyle.forward({})] : []),
    ]) {
      expect(call).toThrow(
        /Stillwind style(?:\.(?:ref|anchorScope|keyframes|viewTransition|when|cva|media|css|forward))?\(\) is a compile-time macro.*integrations\.md/,
      );
    }
  }, 30_000);
}
