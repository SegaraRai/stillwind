import { expect, test, vi } from "vite-plus/test";

import { stillwindClsxChecked, stillwindRefClassChecked } from "./development.ts";
import * as reactDevelopment from "./react-development.ts";
import * as react from "./react.ts";
import * as svelte from "./svelte.ts";

test("checked ref reads evaluate .c once and reject malformed ref records", () => {
  const read = vi.fn(() => "marker");
  expect(
    stillwindRefClassChecked({
      get c() {
        return read();
      },
    }),
  ).toBe("marker");
  expect(read).toHaveBeenCalledTimes(1);
  for (const value of [null, undefined, "marker", {}, { c: 7 }, { c: "" }]) {
    expect(() => stillwindRefClassChecked(value)).toThrow(/class token in .c/);
  }
});

test("checked class concatenation retains external-class input conventions", () => {
  expect(
    stillwindClsxChecked("a", [false, true, null, undefined, 7, 1n, { b: true, c: false }]),
  ).toBe("a 7 1 b");
  for (const value of [Symbol("class"), () => "class", ["a", Symbol("class")]]) {
    expect(() => stillwindClsxChecked(value)).toThrow(/unsupported class value/);
  }
});

test("production and development runtimes share keys and valid materialization", () => {
  const fragment = react.stillwindToFragment({
    className: "a",
    style: { color: "red" },
  });
  expect(reactDevelopment.stillwindToFragmentChecked(fragment)).toEqual(fragment);
  expect(reactDevelopment.stillwindMergeChecked(fragment)).toEqual(react.stillwindMerge(fragment));
  expect(() => reactDevelopment.stillwindToFragmentChecked(null)).toThrow(
    /materialized props object/,
  );
  expect(() =>
    reactDevelopment.stillwindMergeChecked({
      [react.stillwindClassKey]: 7,
    }),
  ).toThrow(/non-string/);
  expect(() =>
    reactDevelopment.stillwindMergeChecked({
      [svelte.stillwindStyleKey]: {},
    }),
  ).toThrow(/different runtime/);
  expect(react.stillwindMerge({ [react.stillwindClassKey]: "a" })).toEqual({ className: "a" });
});
