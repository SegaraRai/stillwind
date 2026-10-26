import { NativeDesignSystemLoader, __napiBindingTarget } from "@stillwind/native";
import { expect, inject, test } from "vite-plus/test";

import { budgets } from "../performance/budgets.ts";
import { canonicalCases, canonicalMixedGroup } from "./canonical-inputs.ts";

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
  const preparationBudget = budgets.canonical.preparation;
  const hardChecks: { scenario: string; timeMs: number; hardMs: number }[] = [
    {
      scenario: "load-css-and-index",
      timeMs: preparation[4],
      hardMs: preparationBudget.hard,
    },
  ];
  if (preparation[4] >= preparationBudget.soft) {
    await context.annotate(
      `load-css-and-index: median ${preparation[4]}ms exceeds ${preparationBudget.soft}ms`,
      "performance-soft",
    );
  }
  for (const { name, candidate, expected } of canonicalCases.filter(
    ({ name }) => name !== "named",
  )) {
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
    const budget = budgets.canonical[name];
    if (samples[50] >= budget.soft) {
      await context.annotate(
        `${candidate}: median ${samples[50]}ms exceeds ${budget.soft}ms`,
        "performance-soft",
      );
    }
    measurements.push({ scenario: candidate, samples });
    hardChecks.push({ scenario: candidate, timeMs: samples[95], hardMs: budget.hard });
  }
  for (const { scenario, timeMs, hardMs } of hardChecks) {
    expect.soft(timeMs, scenario).toBeLessThan(hardMs);
  }
});

test("browser WASM keeps unrelated utilities separate within one group", async (context) => {
  const loader = new NativeDesignSystemLoader();
  loader.provide(0, "theme.css", inject("canonicalBrowserCss"), "file:///theme.css");
  const samples: number[] = [];
  let answer: string[] = [];
  for (let i = 0; i < 10; i++) {
    const query = loader.editorQuery();
    const start = performance.now();
    answer = query.canonicalCandidates([...canonicalMixedGroup]);
    if (i > 0) {
      samples.push(performance.now() - start);
    }
  }
  samples.sort((a, b) => a - b);
  expect(answer).toEqual([...canonicalMixedGroup]);
  context.task.meta.canonicalBrowser = {
    userAgent: navigator.userAgent,
    measurements: [
      {
        scenario: "mixed-group",
        samples,
      },
    ],
  };
  const budget = budgets.canonical.mixedGroup;
  if (samples[4] >= budget.soft) {
    await context.annotate(
      `mixed group: median ${samples[4]}ms exceeds ${budget.soft}ms`,
      "performance-soft",
    );
  }
  expect(samples[4]).toBeLessThan(budget.hard);
});
