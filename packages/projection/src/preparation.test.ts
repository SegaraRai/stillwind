import { expect, test } from "vite-plus/test";

import { originalRange } from "./preparation.ts";

test("original ranges reject surrogate interiors and invalid bounds", () => {
  const source = "日本語🧭\r\nx";
  expect(originalRange(source, 3, 5)).toEqual({ start: 3, end: 5 });
  expect(originalRange(source, 5, 8)).toEqual({ start: 5, end: 8 });
  for (const [start, end] of [
    [3, 4],
    [4, 5],
    [-1, 2],
    [0, 9],
    [5, 3],
    [0.5, 1],
    [Number.NaN, 1],
  ]) {
    expect(originalRange(source, start, end)).toBeUndefined();
  }
  expect(originalRange(source)).toBeUndefined();
});
