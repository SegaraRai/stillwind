import { expect, test } from "vite-plus/test";

import * as astro from "./astro.ts";
import * as preact from "./preact.ts";
import * as react from "./react.ts";
import * as solid from "./solid.ts";
import * as svelte from "./svelte.ts";
import * as vue from "./vue.ts";

test("CVA selects defaults, boolean and numeric options, and every matching compound", () => {
  const choose = react.stillwindCva("base", {
    variants: {
      size: {
        small: "small",
        large: "large",
        "": "empty",
      },
      disabled: {
        true: "disabled",
        false: "enabled",
      },
      count: {
        0: "zero",
        1: "one",
      },
    },
    defaultVariants: {
      size: "small",
      disabled: false,
      count: 1,
    },
    compoundVariants: [
      {
        size: ["small", "large"],
        disabled: false,
        class: "interactive",
      },
      {
        size: "large",
        count: 0,
        className: "big-zero",
      },
      {
        size: null,
        class: "no-size",
      },
      {
        size: "",
        class: "empty-condition",
      },
    ],
  });
  expect(choose()).toEqual({ className: "base small enabled one interactive" });
  expect(choose({ size: undefined, disabled: true, count: 0 })).toEqual({
    className: "base small disabled zero",
  });
  expect(
    choose({ size: "large", count: 0, class: ["extra", false], className: { active: true } }),
  ).toEqual({
    className: "base large enabled zero interactive big-zero extra active",
  });
  expect(choose({ size: null })).toEqual({ className: "base enabled one no-size" });
  expect(choose({ size: "" })).toEqual({ className: "base small enabled one empty-condition" });
  expect(choose({ size: "unknown", ignored: "not-forwarded" })).toEqual({
    className: "base enabled one",
  });
});

test("CVA ignores inherited options and supports base-only and empty factories", () => {
  const options = Object.assign(
    Object.create({
      inherited: "wrong",
    }),
    {
      own: "right",
    },
  );
  const choose = react.stillwindCva(undefined, {
    variants: {
      kind: options,
    },
  });
  expect(choose({ kind: "inherited" })).toEqual({});
  expect(choose({ kind: "own" })).toEqual({ className: "right" });
  expect(react.stillwindCva()()).toEqual({});
  expect(react.stillwindCva("base")({ className: "extra" })).toEqual({ className: "base extra" });
  expect(react.stillwindCva("base", { compoundVariants: [{ class: "unused" }] })()).toEqual({
    className: "base",
  });
});

test("CVA merges custom properties without mutating saved fragments or leaking channels", () => {
  for (const [runtime, classProperty, stringStyle] of [
    [react, "className", false],
    [preact, "class", false],
    [solid, "class", false],
    [vue, "class", false],
    [svelte, "class", true],
    [astro, "class", true],
  ] as const) {
    const baseStyle = { "--tone": "blue", "--size": "4" };
    const optionStyle = { "--tone": "red" };
    const base = {
      [runtime.stillwindClassKey]: "base",
      [runtime.stillwindCustomPropertiesKey]: baseStyle,
    };
    const option = {
      [runtime.stillwindClassKey]: "option",
      [runtime.stillwindCustomPropertiesKey]: optionStyle,
    };
    const choose = runtime.stillwindCva(base, {
      variants: {
        kind: {
          plain: null,
          colored: [option],
        },
      },
      compoundVariants: [
        {
          kind: "colored",
          class: "compound",
        },
      ],
    });
    const result = choose({ kind: "colored" });
    expect(result).toEqual({
      [classProperty]: "base option compound",
      style: stringStyle ? "--tone:red;--size:4" : { "--tone": "red", "--size": "4" },
    });
    expect(Object.getOwnPropertySymbols(result)).toEqual([]);
    result[classProperty] = "changed";
    if (typeof result.style === "object" && result.style !== null) {
      (result.style as Record<string, string>)["--tone"] = "changed";
    }
    expect(choose({ kind: "colored" })[classProperty]).toBe("base option compound");
    expect(choose({ kind: "plain" }).style).toEqual(
      stringStyle ? "--tone:blue;--size:4" : { "--tone": "blue", "--size": "4" },
    );
    expect(baseStyle["--tone"]).toBe("blue");
    expect(optionStyle["--tone"]).toBe("red");
  }
});
