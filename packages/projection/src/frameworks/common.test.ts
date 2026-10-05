import { expect, test } from "vite-plus/test";

import { originalRange, PreparationError, sourceMapV3, sourceRegions } from "./common.ts";

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

test("invalid converter metadata produces a known projection failure with its identity", () => {
  const converter = {
    name: "converter",
    version: "1.0.0",
    optionsFingerprint: "options",
  };
  const operations = [
    () => sourceMapV3("{", converter),
    () =>
      sourceMapV3(
        {
          sources: [],
        },
        converter,
      ),
    () =>
      sourceRegions(
        "text",
        [
          {
            id: 0,
            start: 0,
            end: 10,
          },
        ],
        0,
        converter,
      ),
  ];
  for (const operation of operations) {
    expect(operation).toThrow(PreparationError);
    expect(operation).toThrow(
      expect.objectContaining({
        issue: expect.objectContaining({ kind: "projection" }),
        converter,
      }),
    );
  }
});
