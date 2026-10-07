import { NativeDesignSystemLoader, __napiBindingTarget } from "@stillwind/native";
import { expect, inject, test } from "vite-plus/test";

declare module "vite-plus/test" {
  interface ProvidedContext {
    canonicalBrowserCss: string;
  }
}

test("browser WASM prepares each revision once and answers uncached canonical queries", async (context) => {
  expect(crossOriginIsolated).toBe(true);
  expect(__napiBindingTarget).toBe("wasm32-wasi");
  const css = inject("canonicalBrowserCss");
  const load = () => {
    const loader = new NativeDesignSystemLoader();
    loader.provide(0, "theme.css", css, "file:///theme.css");
    return loader;
  };
  let loader = load();
  const preparation: number[] = [];
  for (let i = 0; i < 9; i++) {
    const start = performance.now();
    loader = load();
    preparation.push(performance.now() - start);
  }
  preparation.sort((a, b) => a - b);
  const measurements = [{ scenario: "load-css-and-index", samples: preparation }];
  context.task.meta.canonicalBrowser = { userAgent: navigator.userAgent, measurements };
  expect(preparation[4]).toBeLessThan(300);
  for (const [candidate, expected] of [
    ["p-[16px]", "p-4"],
    ["bg-[#f00]", "bg-brand"],
    ["bg-[#123456]", null],
    ["[border-color:#123456]", null],
    ["bg-[#f00]/50", "bg-brand/50"],
    ["bg-[#123456]/50", null],
    ["[&:hover]:p-4", null],
  ] as const) {
    const samples: number[] = [];
    let answer: string | null = null;
    for (let i = 0; i < 120; i++) {
      const query = loader.editorQuery();
      const start = performance.now();
      answer = query.canonicalCandidate(candidate);
      const elapsed = performance.now() - start;
      if (i >= 20) {
        samples.push(elapsed);
      }
    }
    samples.sort((a, b) => a - b);
    expect(answer).toBe(expected);
    expect(samples[95], candidate).toBeLessThan(1);
    if (samples[50] >= 0.1) {
      await context.annotate(
        `${candidate}: median ${samples[50]}ms exceeds 0.1ms`,
        "performance-soft",
      );
    }
    measurements.push({ scenario: candidate, samples });
  }
});
