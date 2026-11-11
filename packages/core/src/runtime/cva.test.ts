import { expect, test } from "vite-plus/test";

import * as checked from "./checked.ts";
import * as formats from "./formats.ts";

const outputs = [
  {
    cva: formats.stillwindClassNameObjectCva,
    cvaChecked: checked.stillwindClassNameObjectCvaChecked,
    classKey: formats.stillwindClassNameObjectClassKey,
    customPropertiesKey: formats.stillwindClassNameObjectCustomPropertiesKey,
    classProperty: "className",
    stringStyle: false,
  },
  {
    cva: formats.stillwindClassObjectCva,
    cvaChecked: checked.stillwindClassObjectCvaChecked,
    classKey: formats.stillwindClassObjectClassKey,
    customPropertiesKey: formats.stillwindClassObjectCustomPropertiesKey,
    classProperty: "class",
    stringStyle: false,
  },
  {
    cva: formats.stillwindClassStringCva,
    cvaChecked: checked.stillwindClassStringCvaChecked,
    classKey: formats.stillwindClassStringClassKey,
    customPropertiesKey: formats.stillwindClassStringCustomPropertiesKey,
    classProperty: "class",
    stringStyle: true,
  },
] as const;

test("CVA selects defaults, boolean and numeric options, and every matching compound", () => {
  const choose = formats.stillwindClassNameObjectCva("base", {
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
  const choose = formats.stillwindClassNameObjectCva(undefined, {
    variants: {
      kind: options,
    },
  });
  expect(choose({ kind: "inherited" })).toEqual({});
  expect(choose({ kind: "own" })).toEqual({ className: "right" });
  expect(formats.stillwindClassNameObjectCva()()).toEqual({});
  expect(formats.stillwindClassNameObjectCva("base")({ className: "extra" })).toEqual({
    className: "base extra",
  });
  expect(
    formats.stillwindClassNameObjectCva("base", { compoundVariants: [{ class: "unused" }] })(),
  ).toEqual({
    className: "base",
  });
});

test("checked CVA factories share production channels and validate selected inputs", () => {
  for (const { cva, cvaChecked, classKey, customPropertiesKey } of outputs) {
    const base = {
      [classKey]: "base",
      [customPropertiesKey]: {
        "--size": "4",
      },
    };
    const foreign = {
      [Symbol("foreign")]: "wrong",
    };
    const config = {
      variants: {
        kind: {
          valid: {
            [classKey]: "selected",
          },
          foreign,
          malformed: {
            [classKey]: 7,
          },
        },
      },
      defaultVariants: {
        kind: "valid",
      },
      compoundVariants: [
        {
          kind: "valid",
          class: {
            [customPropertiesKey]: { "--tone": "red" },
          },
        },
      ],
    };
    const choose = cvaChecked(base, config);
    const props = {
      kind: undefined,
      class: ["extra", false],
      className: {
        active: true,
      },
    };
    expect(choose(props)).toEqual(cva(base, config)(props));
    expect(() => choose({ kind: "foreign" })).toThrow(/different runtime/);
    expect(() => choose({ kind: "malformed" })).toThrow(/non-string class/);
    for (const invalidProps of [
      {
        class: Symbol("class"),
      },
      {
        className: [() => "class"],
      },
    ]) {
      expect(() => choose(invalidProps)).toThrow(/unsupported class value/);
    }
    expect(() =>
      cvaChecked(undefined, {
        variants: {},
        compoundVariants: [
          {
            class: foreign,
          },
        ],
      })(),
    ).toThrow(/different runtime/);
  }
});

test("CVA merges custom properties without mutating saved fragments or leaking channels", () => {
  for (const { cva, classKey, customPropertiesKey, classProperty, stringStyle } of outputs) {
    const baseStyle = { "--tone": "blue", "--size": "4" };
    const optionStyle = { "--tone": "red" };
    const base = {
      [classKey]: "base",
      [customPropertiesKey]: baseStyle,
    };
    const option = {
      [classKey]: "option",
      [customPropertiesKey]: optionStyle,
    };
    const choose = cva(base, {
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
