import * as react from "stillwind/runtime/class-name-object";
import * as reactDevelopment from "stillwind/runtime/class-name-object-development";
import * as classObject from "stillwind/runtime/class-object";
import * as svelte from "stillwind/runtime/class-string";
import * as svelteDevelopment from "stillwind/runtime/class-string-development";
import { expect, test } from "vite-plus/test";

test("react merges fragments, arrays, and falsy values in argument order", () => {
  const frozenBase = Object.freeze({ className: "a" });
  const frozenNext = Object.freeze({ className: "b" });
  const delta = Object.freeze({
    [react.stillwindClassKey]: "own",
  });
  expect(
    react.stillwindMerge(
      react.stillwindToFragment(frozenBase),
      react.stillwindToFragment(frozenNext),
      delta,
    ),
  ).toStrictEqual({
    className: "a b own",
  });
  expect(
    react.stillwindMerge(
      react.stillwindToFragment(frozenBase),
      react.stillwindToFragment(frozenNext),
    ),
  ).toStrictEqual({
    className: "a b",
  });
  expect(
    Object.getOwnPropertySymbols(react.stillwindMerge(react.stillwindToFragment(frozenBase))),
  ).toStrictEqual([]);
  const first = { [react.stillwindClassKey]: "_a" };
  const second = { [react.stillwindClassKey]: "_b" };
  expect(
    react.stillwindMerge(first, null, [
      second,
      false,
      [
        [
          {
            [react.stillwindClassKey]: "_c",
          },
        ],
      ],
    ]),
  ).toStrictEqual({
    className: "_a _b _c",
  });
  expect(react.stillwindMerge()).toStrictEqual({});
  expect(react.stillwindMerge(undefined, false)).toStrictEqual({});
});

test("later style payloads win without leaking symbol keys", () => {
  const merged = react.stillwindMerge(
    {
      [react.stillwindClassKey]: "_a",
      [react.stillwindStyleKey]: { "--x": "1", "--y": "1" },
    },
    {
      [react.stillwindStyleKey]: { "--y": "2" },
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
    svelte.stillwindMerge(
      {
        [svelte.stillwindClassKey]: "_a",
        [svelte.stillwindStyleKey]: { "--x": "1", "--y": "2" },
      },
      {
        [svelte.stillwindStyleKey]: "--y:3",
      },
    ),
  ).toStrictEqual({
    class: "_a",
    style: "--y:3",
  });
  expect(
    svelte.stillwindMerge({
      [svelte.stillwindClassKey]: "_z",
      [svelte.stillwindStyleKey]: { "--x": "1" },
    }),
  ).toStrictEqual({
    class: "_z",
    style: "--x:1",
  });
});

test("toFragment keeps only the class of a composition base", () => {
  const fragment = react.stillwindToFragment({
    className: "_base",
    style: {
      "--x": "1",
    },
  });
  expect(fragment).toStrictEqual({
    [react.stillwindClassKey]: "_base",
  });
  expect(react.stillwindToFragment(fragment)).toStrictEqual(fragment);
  expect(
    react.stillwindMerge(fragment, {
      [react.stillwindClassKey]: "_own",
    }),
  ).toStrictEqual({
    className: "_base _own",
  });
  expect(
    svelte.stillwindToFragment({
      class: "_base",
      style: "--x:1",
    }),
  ).toStrictEqual({
    [svelte.stillwindClassKey]: "_base",
  });
  expect(
    react.stillwindMerge(
      react.stillwindToFragment({
        class: "_vue",
      }),
    ),
  ).toStrictEqual({});
});

test("development checks reject foreign fragments and unconverted props objects", () => {
  expect(() =>
    reactDevelopment.stillwindMergeChecked({
      [svelte.stillwindClassKey]: "_x",
    }),
  ).toThrow(/different runtime instance/);
  expect(() =>
    reactDevelopment.stillwindMergeChecked({
      className: "_x",
    }),
  ).toThrow(/stillwindToFragment/);
  expect(() =>
    svelteDevelopment.stillwindMergeChecked({
      class: "_x",
    }),
  ).toThrow(/stillwindToFragment/);
});

test("class concatenation keeps falsy and non-string fragments empty", () => {
  expect(
    react.stillwindMerge(
      {
        [react.stillwindClassKey]: "",
      },
      {
        [react.stillwindClassKey]: undefined,
      },
    ),
  ).toStrictEqual({});
});

test("custom properties merge independently and coexist with generated styles", () => {
  for (const runtime of [react, classObject, svelte]) {
    const key = runtime.stillwindCustomPropertiesKey;
    const result = runtime.stillwindMerge(
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
        [runtime.stillwindStyleKey]: {
          "view-transition-name": "card",
        },
      },
    );
    expect(result.style).toStrictEqual(
      runtime !== svelte
        ? {
            "view-transition-name": "card",
            "--progress": "0.5",
            "--image": "url(image.png)",
          }
        : "view-transition-name:card;--progress:0.5;--image:url(image.png)",
    );
    expect(Object.getOwnPropertySymbols(result)).toStrictEqual([]);
    expect(
      runtime.stillwindMerge(
        runtime.stillwindToFragment({
          [key]: {
            "--x": "1",
          },
        }),
      ),
    ).toStrictEqual({});
  }
});

test("CSS values preserve strings and zero, encode boolean switches and quote URL objects", () => {
  expect(react.stillwindCssValue(0)).toBe("0");
  expect(react.stillwindCssValue(false)).toBe("initial");
  expect(react.stillwindCssValue(true)).toBe(" ");
  expect(react.stillwindCssValue("calc(1px + 2px)")).toBe("calc(1px + 2px)");
  expect(react.stillwindCssValue(new URL("https://example.com/image.png"))).toBe(
    'url("https://example.com/image.png")',
  );
});

test("boolean switches retain whitespace and initial in framework style props", () => {
  for (const runtime of [react, classObject, svelte]) {
    for (const enabled of [true, false]) {
      const value = runtime.stillwindCssValue(enabled);
      const props = runtime.stillwindMerge({
        [runtime.stillwindCustomPropertiesKey]: {
          "--enabled": value,
        },
      });
      expect(props.style).toStrictEqual(
        runtime !== svelte
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
