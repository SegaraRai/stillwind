import { expect, test } from "vite-plus/test";

import * as react from "./class-name-object.ts";
import * as vue from "./class-object.ts";
import * as svelte from "./class-string.ts";

test.each([
  ["class-name-object", react, "className"],
  ["class-object", vue, "class"],
  ["class-string", svelte, "class"],
] as const)(
  "%s composes either class prop and prefers its output prop",
  (_, runtime, classProperty) => {
    for (const base of [
      {
        class: "_base",
        style: { "--tone": "red" },
      },
      {
        className: "_base",
        style: "--tone:red",
      },
    ]) {
      expect(runtime.stillwindToFragment(Object.freeze(base))).toStrictEqual({
        [runtime.stillwindClassKey]: "_base",
      });
    }
    const otherProperty = classProperty === "class" ? "className" : "class";
    for (const preferred of ["_preferred", ""]) {
      expect(
        runtime.stillwindToFragment({
          [classProperty]: preferred,
          [otherProperty]: "_fallback",
        }),
      ).toStrictEqual({
        [runtime.stillwindClassKey]: preferred,
      });
    }
    for (const missing of [undefined, null]) {
      expect(
        runtime.stillwindToFragment({
          [classProperty]: missing,
          [otherProperty]: "_fallback",
        }),
      ).toStrictEqual({
        [runtime.stillwindClassKey]: "_fallback",
      });
    }
  },
);

test("style value conversion matches merge serialization", () => {
  for (const runtime of [react, svelte, vue]) {
    for (const value of [
      undefined,
      null,
      false,
      true,
      0,
      1,
      "",
      "a b",
      {},
      { "--x": 1 },
      ["a"],
      Symbol("value"),
    ]) {
      expect(runtime.stillwindStyleValue(value)).toStrictEqual(
        runtime.stillwindMerge({
          [runtime.stillwindStyleKey]: value,
        }).style,
      );
    }
  }
});
