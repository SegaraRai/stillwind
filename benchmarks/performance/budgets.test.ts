import { expect, test, vi, type BenchResult } from "vite-plus/test";

import { assertBudget, budgets } from "./budgets.ts";

test("soft exceedances annotate without failing, and hard exceedances fail", async () => {
  const context = { task: { meta: {} }, annotate: vi.fn().mockResolvedValue(undefined) };
  const result = (median: number) =>
    ({ latency: { p50: median, max: median + 1, samplesCount: 3 } }) as BenchResult;
  await assertBudget(context, "edit", result(9), {
    soft: 10,
    hard: 20,
  });
  expect(context.annotate).not.toHaveBeenCalled();
  await assertBudget(context, "edit", result(10), {
    soft: 10,
    hard: 20,
  });
  expect(context.annotate).toHaveBeenCalledWith(
    expect.stringContaining("soft target 10ms"),
    "performance-soft",
  );
  await expect(
    assertBudget(context, "edit", result(20), {
      soft: 10,
      hard: 20,
    }),
  ).rejects.toThrow("hard limit 20ms");
  expect(context.task.meta).toMatchObject({
    performanceBudgets: [
      expect.objectContaining({ softExceeded: false }),
      expect.objectContaining({ softExceeded: true }),
      expect.objectContaining({ medianMs: 20 }),
    ],
  });
});

test("all declared sizes have positive soft targets below their hard limits", () => {
  for (const sizes of Object.values(budgets)) {
    for (const budget of Object.values(sizes)) {
      expect(budget.soft).toBeGreaterThan(0);
      expect(budget.hard).toBeGreaterThan(budget.soft);
    }
  }
  expect(budgets.build[1000].soft).toBe(5000);
  expect(budgets.startup[1000].soft).toBe(1000);
  expect(budgets.filesystemEdit[1000].soft).toBe(100);
});
