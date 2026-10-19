import { expect, test } from "vite-plus/test";

import { cssClassNames, selectorClassNames, utilityRules, withAlpha } from "./css.ts";

/** Collects the utility bodies that `utilityRules` registers. */
function rules(input: unknown): [string, string][] {
  const registered: [string, string][] = [];
  utilityRules(
    input,
    (name, body) => registered.push([name, body]),
    () => {},
  );
  return registered;
}

test("plugin selectors register every class outside conditions and attribute selectors", () => {
  expect(
    rules({
      ".form-input, .form-select:not(.ghost)": {
        color: "red",
      },
      "[data-x='.no'] .w-1\\/2": {
        width: "50%",
      },
      ".outer .inner": {
        color: "blue",
      },
      "li:nth-child(2 of .selected) .item:nth-last-child(1 of .last)": {
        color: "green",
      },
    }),
  ).toEqual([
    ["form-input", "color:red;"],
    ["form-select", "&:not(.ghost){color:red;}"],
    ["w-1/2", "[data-x='.no'] &{width:50%;}"],
    ["outer", "& .inner{color:blue;}"],
    ["inner", ".outer &{color:blue;}"],
    ["item", "li:nth-child(2 of .selected) &:nth-last-child(1 of .last){color:green;}"],
  ]);
  expect(() =>
    rules({
      ".item, .Upper": {
        color: "red",
      },
    }),
  ).toThrow("requires a class selector");
});

test("nested keyframes and properties stay in the utility body with their conditions", () => {
  const globals: string[] = [];
  const registered: [string, string][] = [];
  utilityRules(
    {
      ".spin": {
        animation: "turn 1s",
        "@media (min-width: 1px)": {
          "@keyframes turn": {
            to: {
              rotate: "1turn",
            },
          },
          "@property --turn": {
            syntax: '"*"',
            inherits: "false",
          },
        },
      },
      "@keyframes shared": {
        to: {
          opacity: "1",
        },
      },
      "@property --shared": {
        syntax: '"*"',
        inherits: "false",
      },
    },
    (name, body) => registered.push([name, body]),
    (css) => globals.push(css),
  );
  expect(registered).toEqual([
    ["spin", "animation:turn 1s;"],
    ["spin", "@media (min-width: 1px){@keyframes turn{to{rotate:1turn;}}}"],
    ["spin", '@media (min-width: 1px){@property --turn{syntax:"*";inherits:false;}}'],
  ]);
  expect(globals).toEqual([
    "@keyframes shared{to{opacity:1;}}",
    '@property --shared{syntax:"*";inherits:false;}',
  ]);
});

test("empty at-rule objects become statements", () => {
  expect(
    rules({
      ".card": {
        "@apply p-4": {},
        color: "red",
      },
    }),
  ).toEqual([["card", "@apply p-4;color:red;"]]);
});

test("nested selector objects become one target per selector", () => {
  expect(
    rules({
      ".btn": {
        color: "red",
        "&:hover, .icon": {
          color: "blue",
        },
      },
    }),
  ).toEqual([
    ["btn", "color:red;"],
    ["btn", "&:hover{color:blue;}"],
    ["btn", "& .icon{color:blue;}"],
  ]);
});

test("alpha values follow Tailwind's withAlpha", () => {
  expect(withAlpha("red", "0.5")).toBe("color-mix(in oklab, red 50%, transparent)");
  expect(withAlpha("red", "50%")).toBe("color-mix(in oklab, red 50%, transparent)");
  // Tailwind multiplies every number, so 50 means 5000%.
  expect(withAlpha("red", "50")).toBe("color-mix(in oklab, red 5000%, transparent)");
  expect(withAlpha("red", "1")).toBe("red");
  expect(withAlpha("red", "var(--a)")).toBe("color-mix(in oklab, red var(--a), transparent)");
});

test("selectors name classes at every position and through whole class attribute matches", () => {
  expect(
    selectorClassNames(String.raw`.a &:not(.b):has(> .c\/d), [class~="e"] [class*="f"]`),
  ).toEqual(["a", "b", "c/d", "e"]);
  expect(selectorClassNames(`[data-x=".a"] /* .b */ #c .5`)).toEqual([]);
});

test("CSS text names classes only in style rule selectors", () => {
  expect(
    cssClassNames(`color:red;.a &{content:".b"}@media (width>=40.5rem){.c &{margin:0}}`),
  ).toEqual(["a", "c"]);
});
