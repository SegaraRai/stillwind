import { expect, test } from "vite-plus/test";

import { utilityRules, withAlpha } from "./css.ts";

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

test("plugin selectors register their first class outside :not() and attribute selectors", () => {
  expect(
    rules({
      ".form-input, .form-select:not(.ghost)": {
        color: "red",
      },
      "[data-x='.no'] .w-1\\/2": {
        width: "50%",
      },
    }),
  ).toEqual([
    ["form-input", "color:red;"],
    ["form-select", "&:not(.ghost){color:red;}"],
    ["w-1/2", "[data-x='.no'] &{width:50%;}"],
  ]);
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
