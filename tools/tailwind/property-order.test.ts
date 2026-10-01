import { expect, test } from "vite-plus/test";

import { hasEquivalentDeclarationOrder } from "./property-order.ts";

test("accepts reordered declarations that affect disjoint properties", () => {
  expect(
    hasEquivalentDeclarationOrder(
      ".button{display:inline-flex;background-color:red;padding-inline:1rem}",
      ".button{display:inline-flex;padding-inline:1rem;background-color:red}",
    ),
  ).toBe(true);
});

test("rejects reordered values for the same property", () => {
  expect(
    hasEquivalentDeclarationOrder(".button{color:red;color:blue}", ".button{color:blue;color:red}"),
  ).toBe(false);
});

test("rejects reordered shorthand and longhand declarations", () => {
  expect(
    hasEquivalentDeclarationOrder(
      ".button{margin:1rem;margin-left:2rem}",
      ".button{margin-left:2rem;margin:1rem}",
    ),
  ).toBe(false);
});

test("still requires declaration values and surrounding CSS structure to match", () => {
  expect(
    hasEquivalentDeclarationOrder(
      ".button{display:block;color:red}",
      ".button{display:flex;color:red}",
    ),
  ).toBe(false);
  expect(
    hasEquivalentDeclarationOrder("@media (width > 1px){.button{color:red}}", ".button{color:red}"),
  ).toBe(false);
});
