import { expect, test, vi } from "vite-plus/test";

import {
  stillwindClassNameObjectMergeChecked,
  stillwindClassNameObjectToFragmentChecked,
  stillwindClsxChecked,
  stillwindRefClassChecked,
} from "./checked.ts";
import {
  stillwindClassNameObjectClassKey,
  stillwindClassNameObjectMerge,
  stillwindClassNameObjectToFragment,
  stillwindClassStringStyleKey,
} from "./formats.ts";

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
  const fragment = stillwindClassNameObjectToFragment({
    className: "a",
    style: { color: "red" },
  });
  expect(stillwindClassNameObjectToFragmentChecked(fragment)).toEqual(fragment);
  expect(stillwindClassNameObjectMergeChecked(fragment)).toEqual(
    stillwindClassNameObjectMerge(fragment),
  );
  expect(() => stillwindClassNameObjectToFragmentChecked(null)).toThrow(
    /materialized props object/,
  );
  expect(() =>
    stillwindClassNameObjectMergeChecked({
      [stillwindClassNameObjectClassKey]: 7,
    }),
  ).toThrow(/non-string/);
  expect(() =>
    stillwindClassNameObjectMergeChecked({
      [stillwindClassStringStyleKey]: {},
    }),
  ).toThrow(/different runtime/);
  expect(stillwindClassNameObjectMerge({ [stillwindClassNameObjectClassKey]: "a" })).toEqual({
    className: "a",
  });
});
