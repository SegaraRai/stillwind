import { expect, test } from "vite-plus/test";

import { analysisConcurrency, groupProjects } from "./project-workers.ts";

test("project groups keep shared targets together and balance distinct targets", () => {
  const targets = [
    ["/a/1.tsx", "/shared.css"],
    ["/b/1.tsx", "/shared.css"],
    ["/c/1.tsx", "/c/2.tsx", "/c/3.tsx", "/c/4.tsx"],
    ["/d/1.tsx"],
    [],
  ];
  // Components by distinct targets: {0, 1}: 3, {2}: 4, {3}: 1, {4}: 0.
  expect(groupProjects(targets, 2)).toEqual([
    [0, 1, 3],
    [2, 4],
  ]);
  expect(groupProjects(targets, 8)).toEqual([[0, 1], [2], [3], [4]]);
  expect(groupProjects(targets, 1)).toEqual([[0, 1, 2, 3, 4]]);
  expect(groupProjects([], 4)).toEqual([]);
});

test("analysis concurrency rejects values that are not positive integers", () => {
  expect(analysisConcurrency(3)).toBe(3);
  expect(analysisConcurrency()).toBeGreaterThanOrEqual(1);
  for (const value of [0, -1, 1.5, Number.NaN]) {
    expect(() => analysisConcurrency(value)).toThrow("positive integer");
  }
});
