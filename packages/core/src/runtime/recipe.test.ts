import { expect, test } from "vite-plus/test";

import * as checked from "./checked.ts";
import * as formats from "./formats.ts";

const outputs = [
  {
    recipe: formats.stillwindClassNameObjectRecipe,
    recipeChecked: checked.stillwindClassNameObjectRecipeChecked,
    classKey: formats.stillwindClassNameObjectClassKey,
    customPropertiesKey: formats.stillwindClassNameObjectCustomPropertiesKey,
    classProperty: "className",
    stringStyle: false,
  },
  {
    recipe: formats.stillwindClassObjectRecipe,
    recipeChecked: checked.stillwindClassObjectRecipeChecked,
    classKey: formats.stillwindClassObjectClassKey,
    customPropertiesKey: formats.stillwindClassObjectCustomPropertiesKey,
    classProperty: "class",
    stringStyle: false,
  },
  {
    recipe: formats.stillwindClassStringRecipe,
    recipeChecked: checked.stillwindClassStringRecipeChecked,
    classKey: formats.stillwindClassStringClassKey,
    customPropertiesKey: formats.stillwindClassStringCustomPropertiesKey,
    classProperty: "class",
    stringStyle: true,
  },
] as const;

test("Recipe selects defaults, boolean and numeric options, and every matching compound", () => {
  const choose = formats.stillwindClassNameObjectRecipe("base", {
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

test("Recipe with slots selects props for each slot", () => {
  const card = formats.stillwindClassNameObjectRecipe("root", {
    slots: {
      base: "base-slot",
      icon: "icon",
      label: "label",
    },
    variants: {
      size: {
        small: {
          base: "root-small",
          icon: "icon-small",
        },
        large: "root-large",
      },
      tone: {
        quiet: {
          label: "label-quiet",
        },
        loud: {},
      },
    },
    defaultVariants: {
      size: "small",
    },
    compoundVariants: [
      {
        size: "small",
        tone: "quiet",
        class: {
          icon: "icon-small-quiet",
        },
      },
      {
        size: "large",
        className: "root-large-compound",
      },
    ],
    compoundSlots: [
      {
        slots: ["icon", "label"],
        tone: "quiet",
        class: "quiet-part",
      },
    ],
  });
  const forwarded = formats.stillwindClassNameObjectForward({
    className: "forwarded",
  });
  const slots = (props?: Record<string, unknown>, forward?: unknown) =>
    card(props, forward) as Record<string, (props?: Record<string, unknown>) => unknown>;
  const { base, icon, label } = slots(
    {
      tone: "quiet",
      className: "outer",
    },
    forwarded,
  );
  expect(base!()).toEqual({
    className: "root base-slot root-small outer forwarded",
  });
  expect(icon!()).toEqual({
    className: "icon icon-small icon-small-quiet quiet-part",
  });
  expect(
    label!({
      className: "own",
    }),
  ).toEqual({
    className: "label label-quiet quiet-part own",
  });
  expect(
    icon!({
      size: "large",
      tone: undefined,
    }),
  ).toEqual({
    className: "icon quiet-part",
  });
  const large = slots({
    size: "large",
  });
  expect(large.base!()).toEqual({
    className: "root base-slot root-large root-large-compound",
  });
  expect(large.label!()).toEqual({
    className: "label",
  });
  expect(Object.keys(large)).toEqual(["base", "icon", "label"]);
});

test("Recipe ignores inherited options and supports base-only and empty factories", () => {
  const options = Object.assign(
    Object.create({
      inherited: "wrong",
    }),
    {
      own: "right",
    },
  );
  const choose = formats.stillwindClassNameObjectRecipe(undefined, {
    variants: {
      kind: options,
    },
  });
  expect(choose({ kind: "inherited" })).toEqual({});
  expect(choose({ kind: "own" })).toEqual({ className: "right" });
  expect(formats.stillwindClassNameObjectRecipe()()).toEqual({});
  expect(formats.stillwindClassNameObjectRecipe("base")({ className: "extra" })).toEqual({
    className: "base extra",
  });
  expect(
    formats.stillwindClassNameObjectRecipe("base", { compoundVariants: [{ class: "unused" }] })(),
  ).toEqual({
    className: "base",
  });
});

test("checked recipe factories share production channels and validate selected inputs", () => {
  for (const { recipe, recipeChecked, classKey, customPropertiesKey } of outputs) {
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
    const choose = recipeChecked(base, config);
    const props = {
      kind: undefined,
      class: ["extra", false],
      className: {
        active: true,
      },
    };
    expect(choose(props)).toEqual(recipe(base, config)(props));
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
      recipeChecked(undefined, {
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

test("Recipe merges custom properties without mutating saved fragments or leaking channels", () => {
  for (const { recipe, classKey, customPropertiesKey, classProperty, stringStyle } of outputs) {
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
    const choose = recipe(base, {
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
