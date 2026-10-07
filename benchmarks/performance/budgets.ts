import { expect, type BenchResult, type TestContext } from "vite-plus/test";

/**
 * Limits in milliseconds for a scenario's median time on the reference machine.
 */
export interface Budget {
  /**
   * A target. Reaching or exceeding it adds a note without failing the test.
   */
  soft: number;
  /**
   * An upper limit. Reaching or exceeding it fails the test.
   */
  hard: number;
}

/**
 * Targets for each scenario and input size.
 */
export const budgets = {
  canonical: {
    named: { soft: 0.01, hard: 0.1 },
    spacing: { soft: 0.1, hard: 1 },
    "color-match": { soft: 0.1, hard: 1 },
    "color-miss": { soft: 0.1, hard: 1 },
    "property-miss": { soft: 0.1, hard: 1 },
    "variant-miss": { soft: 0.1, hard: 1 },
    "color-modifier-match": { soft: 0.1, hard: 1 },
    "color-modifier-miss": { soft: 0.1, hard: 1 },
    preparation: { soft: 200, hard: 300 },
    cachedBatch: { soft: 1, hard: 5 },
    repeated100: { soft: 5, hard: 15 },
    unique100: { soft: 15, hard: 30 },
  },
  canonicalPalette: {
    100: { soft: 0.1, hard: 1 },
    1000: { soft: 0.1, hard: 1 },
  },
  build: {
    10: { soft: 250, hard: 750 },
    100: { soft: 1000, hard: 2000 },
    1000: { soft: 5000, hard: 10000 },
  },
  startup: {
    10: { soft: 300, hard: 1000 },
    100: { soft: 500, hard: 1000 },
    1000: { soft: 1000, hard: 2000 },
  },
  transform: {
    10: { soft: 10, hard: 40 },
    100: { soft: 30, hard: 80 },
    1000: { soft: 100, hard: 200 },
  },
  filesystemEdit: {
    10: { soft: 50, hard: 150 },
    100: { soft: 75, hard: 200 },
    1000: { soft: 100, hard: 250 },
  },
  cssStartup: {
    10: { soft: 100, hard: 300 },
    100: { soft: 300, hard: 750 },
    1000: { soft: 1000, hard: 2000 },
  },
  cssTransform: {
    10: { soft: 10, hard: 30 },
    100: { soft: 20, hard: 50 },
    1000: { soft: 100, hard: 200 },
  },
  cssFilesystemEdit: {
    10: { soft: 50, hard: 150 },
    100: { soft: 150, hard: 400 },
    1000: { soft: 1000, hard: 2000 },
  },
  definitions: {
    100: { soft: 10, hard: 50 },
    1000: { soft: 50, hard: 200 },
    10000: { soft: 300, hard: 1000 },
  },
  definitionEdit: {
    100: { soft: 5, hard: 15 },
    1000: { soft: 5, hard: 15 },
    10000: { soft: 5, hard: 15 },
  },
} as const;

declare module "vite-plus/test" {
  interface ProvidedContext {
    performanceFull: boolean;
  }

  interface TaskMeta {
    performanceBudgets?: {
      metric: string;
      medianMs: number;
      maximumMs: number;
      softMs: number;
      hardMs: number;
      softExceeded: boolean;
    }[];
  }
}

/**
 * Records `result` under `metric` in the test metadata and checks its median time.
 * `result` must contain at least three benchmark samples in milliseconds.
 * Reaching `budget.soft` adds a note. Reaching `budget.hard` fails the test.
 * Call this after measurement so reporting and assertions do not affect the timing.
 */
export async function assertBudget(
  context: { task: Pick<TestContext["task"], "meta">; annotate: TestContext["annotate"] },
  metric: string,
  result: BenchResult,
  budget: Budget,
): Promise<void> {
  const medianMs = result.latency.p50;
  const maximumMs = result.latency.max;
  expect(Number.isFinite(medianMs), `${metric}: a finite median is required`).toBe(true);
  expect(
    result.latency.samplesCount,
    `${metric}: three measured samples are required`,
  ).toBeGreaterThanOrEqual(3);
  (context.task.meta.performanceBudgets ??= []).push({
    metric,
    medianMs,
    maximumMs,
    softMs: budget.soft,
    hardMs: budget.hard,
    softExceeded: medianMs >= budget.soft,
  });
  if (medianMs >= budget.soft) {
    await context.annotate(
      `${metric}: median ${medianMs.toFixed(2)}ms exceeds soft target ${budget.soft}ms (hard ${budget.hard}ms)`,
      "performance-soft",
    );
  }
  expect(medianMs, `${metric}: median must stay below hard limit ${budget.hard}ms`).toBeLessThan(
    budget.hard,
  );
}
