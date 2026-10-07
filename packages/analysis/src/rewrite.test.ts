import { expect, test } from "vite-plus/test";

import { applySourceEdits } from "./rewrite.ts";

test("source edits use original UTF-16 ranges and reject invalid plans", () => {
  expect(
    applySourceEdits("🦀abc", [
      { start: 2, end: 3, newText: "long" },
      { start: 4, end: 5, newText: "C" },
    ]),
  ).toBe("🦀longbC");
  expect(() =>
    applySourceEdits("abc", [
      { start: 0, end: 2, newText: "" },
      { start: 1, end: 3, newText: "" },
    ]),
  ).toThrow("overlapping");
  expect(() =>
    applySourceEdits("abc", [
      {
        start: -1,
        end: 0,
        newText: "",
      },
    ]),
  ).toThrow("Invalid");
  expect(() =>
    applySourceEdits("abc", [
      {
        start: 0,
        end: 4,
        newText: "",
      },
    ]),
  ).toThrow("Invalid");
});
