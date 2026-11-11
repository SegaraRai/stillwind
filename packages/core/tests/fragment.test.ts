import * as runtime from "stillwind/runtime";
import * as development from "stillwind/runtime/development";
import { expect, test } from "vite-plus/test";

const outputs = [
  {
    merge: runtime.stillwindClassNameObjectMerge,
    toFragment: runtime.stillwindClassNameObjectToFragment,
    styleKey: runtime.stillwindClassNameObjectStyleKey,
    customPropertiesKey: runtime.stillwindClassNameObjectCustomPropertiesKey,
    stringStyle: false,
  },
  {
    merge: runtime.stillwindClassObjectMerge,
    toFragment: runtime.stillwindClassObjectToFragment,
    styleKey: runtime.stillwindClassObjectStyleKey,
    customPropertiesKey: runtime.stillwindClassObjectCustomPropertiesKey,
    stringStyle: false,
  },
  {
    merge: runtime.stillwindClassStringMerge,
    toFragment: runtime.stillwindClassStringToFragment,
    styleKey: runtime.stillwindClassStringStyleKey,
    customPropertiesKey: runtime.stillwindClassStringCustomPropertiesKey,
    stringStyle: true,
  },
] as const;

test("react merges fragments, arrays, and falsy values in argument order", () => {
  const frozenBase = Object.freeze({ className: "a" });
  const frozenNext = Object.freeze({ className: "b" });
  const delta = Object.freeze({
    [runtime.stillwindClassNameObjectClassKey]: "own",
  });
  expect(
    runtime.stillwindClassNameObjectMerge(
      runtime.stillwindClassNameObjectToFragment(frozenBase),
      runtime.stillwindClassNameObjectToFragment(frozenNext),
      delta,
    ),
  ).toStrictEqual({
    className: "a b own",
  });
  expect(
    runtime.stillwindClassNameObjectMerge(
      runtime.stillwindClassNameObjectToFragment(frozenBase),
      runtime.stillwindClassNameObjectToFragment(frozenNext),
    ),
  ).toStrictEqual({
    className: "a b",
  });
  expect(
    Object.getOwnPropertySymbols(
      runtime.stillwindClassNameObjectMerge(runtime.stillwindClassNameObjectToFragment(frozenBase)),
    ),
  ).toStrictEqual([]);
  const first = { [runtime.stillwindClassNameObjectClassKey]: "_a" };
  const second = { [runtime.stillwindClassNameObjectClassKey]: "_b" };
  expect(
    runtime.stillwindClassNameObjectMerge(first, null, [
      second,
      false,
      [
        [
          {
            [runtime.stillwindClassNameObjectClassKey]: "_c",
          },
        ],
      ],
    ]),
  ).toStrictEqual({
    className: "_a _b _c",
  });
  expect(runtime.stillwindClassNameObjectMerge()).toStrictEqual({});
  expect(runtime.stillwindClassNameObjectMerge(undefined, false)).toStrictEqual({});
});

test("later style payloads win without leaking symbol keys", () => {
  const merged = runtime.stillwindClassNameObjectMerge(
    {
      [runtime.stillwindClassNameObjectClassKey]: "_a",
      [runtime.stillwindClassNameObjectStyleKey]: { "--x": "1", "--y": "1" },
    },
    {
      [runtime.stillwindClassNameObjectStyleKey]: { "--y": "2" },
    },
  );
  expect(merged).toStrictEqual({
    className: "_a",
    style: { "--y": "2" },
  });
  expect(Object.getOwnPropertySymbols(merged)).toStrictEqual([]);
});

test("string-style frameworks serialize the payload and pass strings through", () => {
  expect(
    runtime.stillwindClassStringMerge(
      {
        [runtime.stillwindClassStringClassKey]: "_a",
        [runtime.stillwindClassStringStyleKey]: { "--x": "1", "--y": "2" },
      },
      {
        [runtime.stillwindClassStringStyleKey]: "--y:3",
      },
    ),
  ).toStrictEqual({
    class: "_a",
    style: "--y:3",
  });
  expect(
    runtime.stillwindClassStringMerge({
      [runtime.stillwindClassStringClassKey]: "_z",
      [runtime.stillwindClassStringStyleKey]: { "--x": "1" },
    }),
  ).toStrictEqual({
    class: "_z",
    style: "--x:1",
  });
});

test("toFragment keeps only the class of a composition base", () => {
  const fragment = runtime.stillwindClassNameObjectToFragment({
    className: "_base",
    style: {
      "--x": "1",
    },
  });
  expect(fragment).toStrictEqual({
    [runtime.stillwindClassNameObjectClassKey]: "_base",
  });
  expect(runtime.stillwindClassNameObjectToFragment(fragment)).toStrictEqual(fragment);
  expect(
    runtime.stillwindClassNameObjectMerge(fragment, {
      [runtime.stillwindClassNameObjectClassKey]: "_own",
    }),
  ).toStrictEqual({
    className: "_base _own",
  });
  expect(
    runtime.stillwindClassStringToFragment({
      class: "_base",
      style: "--x:1",
    }),
  ).toStrictEqual({
    [runtime.stillwindClassStringClassKey]: "_base",
  });
  expect(
    runtime.stillwindClassNameObjectMerge(
      runtime.stillwindClassNameObjectToFragment({
        class: "_vue",
      }),
    ),
  ).toStrictEqual({
    className: "_vue",
  });
});

test("development checks reject foreign fragments and unconverted props objects", () => {
  expect(() =>
    development.stillwindClassNameObjectMerge({
      [runtime.stillwindClassStringClassKey]: "_x",
    }),
  ).toThrow(/different runtime instance/);
  for (const merge of [
    development.stillwindClassNameObjectMerge,
    development.stillwindClassStringMerge,
  ]) {
    for (const key of ["class", "className"]) {
      expect(() =>
        merge({
          [key]: "_x",
        }),
      ).toThrow(/ToFragment helper/);
    }
  }
});

test("class concatenation keeps falsy and non-string fragments empty", () => {
  expect(
    runtime.stillwindClassNameObjectMerge(
      {
        [runtime.stillwindClassNameObjectClassKey]: "",
      },
      {
        [runtime.stillwindClassNameObjectClassKey]: undefined,
      },
    ),
  ).toStrictEqual({});
});

test("custom properties merge independently and coexist with generated styles", () => {
  for (const { merge, toFragment, styleKey, customPropertiesKey: key, stringStyle } of outputs) {
    const result = merge(
      {
        [key]: {
          "--progress": "0",
          "--image": "url(image.png)",
        },
      },
      false,
      [
        {
          [key]: {
            "--progress": "0.5",
          },
        },
      ],
      {
        [styleKey]: {
          "view-transition-name": "card",
        },
      },
    );
    expect(result.style).toStrictEqual(
      !stringStyle
        ? {
            "view-transition-name": "card",
            "--progress": "0.5",
            "--image": "url(image.png)",
          }
        : "view-transition-name:card;--progress:0.5;--image:url(image.png)",
    );
    expect(Object.getOwnPropertySymbols(result)).toStrictEqual([]);
    expect(
      merge(
        toFragment({
          [key]: {
            "--x": "1",
          },
        }),
      ),
    ).toStrictEqual({});
  }
});

test("CSS values preserve strings and zero, encode boolean switches and quote URL objects", () => {
  expect(runtime.stillwindCssValue(0)).toBe("0");
  expect(runtime.stillwindCssValue(false)).toBe("initial");
  expect(runtime.stillwindCssValue(true)).toBe(" ");
  expect(runtime.stillwindCssValue("calc(1px + 2px)")).toBe("calc(1px + 2px)");
  expect(runtime.stillwindCssValue(new URL("https://example.com/image.png"))).toBe(
    'url("https://example.com/image.png")',
  );
});

test("boolean switches retain whitespace and initial in framework style props", () => {
  for (const { merge, customPropertiesKey, stringStyle } of outputs) {
    for (const enabled of [true, false]) {
      const value = runtime.stillwindCssValue(enabled);
      const props = merge({
        [customPropertiesKey]: {
          "--enabled": value,
        },
      });
      expect(props.style).toStrictEqual(
        !stringStyle
          ? {
              "--enabled": enabled ? " " : "initial",
            }
          : enabled
            ? "--enabled: "
            : "--enabled:initial",
      );
    }
  }
});
