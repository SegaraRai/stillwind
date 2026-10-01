import * as astro from "stillwind/astro/runtime";
import * as react from "stillwind/react/runtime";
import * as svelte from "stillwind/svelte/runtime";
import { expect, test } from "vite-plus/test";

import { createFragmentRuntime } from "../src/runtime/fragment.ts";

test("react merges fragments, arrays, and falsy values in argument order", () => {
  const frozenBase = Object.freeze({ className: "a" });
  const frozenNext = Object.freeze({ className: "b" });
  const delta = Object.freeze({
    [react.stillwindClassKey]: "own",
    "data-state": "ready",
  });
  expect(
    react.stillwindMerge(
      react.stillwindToFragment(frozenBase),
      react.stillwindToFragment(frozenNext),
      delta,
    ),
  ).toStrictEqual({
    className: "a b own",
    "data-state": "ready",
  });
  expect(
    react.stillwindMerge(
      react.stillwindToFragment(frozenBase),
      react.stillwindToFragment(frozenNext),
    ),
  ).toStrictEqual({ className: "a b" });
  expect(
    Object.getOwnPropertySymbols(react.stillwindMerge(react.stillwindToFragment(frozenBase))),
  ).toStrictEqual([]);
  const first = { [react.stillwindClassKey]: "_a", "data-state": "ready" };
  const second = { [react.stillwindClassKey]: "_b" };
  expect(
    react.stillwindMerge(first, null, [second, false, [[{ [react.stillwindClassKey]: "_c" }]]]),
  ).toStrictEqual({
    className: "_a _b _c",
    "data-state": "ready",
  });
  expect(react.stillwindMerge()).toStrictEqual({});
  expect(react.stillwindMerge(undefined, false)).toStrictEqual({});
});

test("later props and style payloads win without leaking symbol keys", () => {
  const merged = react.stillwindMerge(
    {
      [react.stillwindClassKey]: "_a",
      [react.stillwindStyleKey]: { "--x": "1", "--y": "1" },
      title: "a",
    },
    {
      [react.stillwindStyleKey]: { "--y": "2" },
      title: "b",
    },
  );
  expect(merged).toStrictEqual({
    className: "_a",
    title: "b",
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
  ).toStrictEqual({ class: "_a", style: "--y:3" });
  expect(
    astro.stillwindMerge({
      [astro.stillwindClassKey]: "_z",
      [astro.stillwindStyleKey]: { "--x": "1" },
    }),
  ).toStrictEqual({ class: "_z", style: "--x:1" });
});

test("toFragment converts materialized bases and is idempotent", () => {
  const fragment = react.stillwindToFragment({
    className: "_base",
    style: { "--x": "1" },
    "data-a": "1",
  });
  expect(fragment[react.stillwindClassKey]).toBe("_base");
  expect(fragment[react.stillwindStyleKey]).toStrictEqual({ "--x": "1" });
  expect(fragment["data-a"]).toBe("1");
  expect(react.stillwindToFragment(fragment)).toBe(fragment);
  expect(react.stillwindMerge(fragment, { [react.stillwindClassKey]: "_own" })).toStrictEqual({
    className: "_base _own",
    "data-a": "1",
    style: { "--x": "1" },
  });
  const svelteFragment = svelte.stillwindToFragment({
    class: "_base",
    style: "--x:1",
  });
  expect(svelteFragment[svelte.stillwindClassKey]).toBe("_base");
  expect(svelteFragment[svelte.stillwindStyleKey]).toBe("--x:1");
});

test("development checks reject foreign fragments and unconverted props objects", () => {
  const foreign = createFragmentRuntime({
    classProperty: "className",
    styleFormat: "object",
  });
  expect(() => react.stillwindMerge({ [foreign.stillwindClassKey]: "_x" })).toThrow(
    /different runtime instance/,
  );
  expect(() => react.stillwindMerge({ className: "_x" })).toThrow(/stillwindToFragment/);
  expect(() => svelte.stillwindMerge({ class: "_x" })).toThrow(/stillwindToFragment/);
});

test("class concatenation keeps falsy and non-string fragments empty", () => {
  expect(
    react.stillwindMerge(
      { [react.stillwindClassKey]: "" },
      { [react.stillwindClassKey]: undefined },
    ),
  ).toStrictEqual({});
  expect(react.stillwindMerge({ title: "only" })).toStrictEqual({
    title: "only",
  });
});

test("custom properties merge independently and coexist with generated styles", () => {
  for (const runtime of [react, svelte, astro]) {
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
      runtime === react
        ? {
            "view-transition-name": "card",
            "--progress": "0.5",
            "--image": "url(image.png)",
          }
        : "view-transition-name:card;--progress:0.5;--image:url(image.png)",
    );
    expect(Object.getOwnPropertySymbols(result)).toStrictEqual([]);
    const fragment = {
      [key]: { "--x": "1" },
    };
    expect(runtime.stillwindToFragment(fragment)).toBe(fragment);
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
  for (const runtime of [react, svelte, astro]) {
    for (const enabled of [true, false]) {
      const value = runtime.stillwindCssValue(enabled);
      const props = runtime.stillwindMerge({
        [runtime.stillwindCustomPropertiesKey]: {
          "--enabled": value,
        },
      });
      expect(props.style).toStrictEqual(
        runtime === react
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
