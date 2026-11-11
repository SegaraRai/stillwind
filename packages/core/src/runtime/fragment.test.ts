import { expect, test } from "vite-plus/test";

import * as formats from "./formats.ts";

test.each([
  [
    "class-name-object",
    formats.stillwindClassNameObjectToFragment,
    formats.stillwindClassNameObjectClassKey,
    "className",
  ],
  [
    "class-object",
    formats.stillwindClassObjectToFragment,
    formats.stillwindClassObjectClassKey,
    "class",
  ],
  [
    "class-string",
    formats.stillwindClassStringToFragment,
    formats.stillwindClassStringClassKey,
    "class",
  ],
] as const)(
  "%s composes either class prop and prefers its output prop",
  (_, toFragment, classKey, classProperty) => {
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
      expect(toFragment(Object.freeze(base))).toStrictEqual({
        [classKey]: "_base",
      });
    }
    const otherProperty = classProperty === "class" ? "className" : "class";
    for (const preferred of ["_preferred", ""]) {
      expect(
        toFragment({
          [classProperty]: preferred,
          [otherProperty]: "_fallback",
        }),
      ).toStrictEqual({
        [classKey]: preferred,
      });
    }
    for (const missing of [undefined, null]) {
      expect(
        toFragment({
          [classProperty]: missing,
          [otherProperty]: "_fallback",
        }),
      ).toStrictEqual({
        [classKey]: "_fallback",
      });
    }
  },
);

test("style value conversion matches merge serialization", () => {
  for (const [styleValue, merge, styleKey] of [
    [
      formats.stillwindClassNameObjectStyleValue,
      formats.stillwindClassNameObjectMerge,
      formats.stillwindClassNameObjectStyleKey,
    ],
    [
      formats.stillwindClassObjectStyleValue,
      formats.stillwindClassObjectMerge,
      formats.stillwindClassObjectStyleKey,
    ],
    [
      formats.stillwindClassStringStyleValue,
      formats.stillwindClassStringMerge,
      formats.stillwindClassStringStyleKey,
    ],
  ] as const) {
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
      expect(styleValue(value)).toStrictEqual(
        merge({
          [styleKey]: value,
        }).style,
      );
    }
  }
});
