import assert from "node:assert/strict";
import test from "node:test";

import { hasEquivalentDeclarationOrder } from "./lib/property-order-parity.ts";

void test("accepts reordered declarations that affect disjoint properties", () => {
  assert.equal(
    hasEquivalentDeclarationOrder(
      ".button{display:inline-flex;background-color:red;padding-inline:1rem}",
      ".button{display:inline-flex;padding-inline:1rem;background-color:red}",
    ),
    true,
  );
});

void test("rejects reordered values for the same property", () => {
  assert.equal(
    hasEquivalentDeclarationOrder(".button{color:red;color:blue}", ".button{color:blue;color:red}"),
    false,
  );
});

void test("rejects reordered shorthand and longhand declarations", () => {
  assert.equal(
    hasEquivalentDeclarationOrder(
      ".button{margin:1rem;margin-left:2rem}",
      ".button{margin-left:2rem;margin:1rem}",
    ),
    false,
  );
});

void test("still requires declaration values and surrounding CSS structure to match", () => {
  assert.equal(
    hasEquivalentDeclarationOrder(
      ".button{display:block;color:red}",
      ".button{display:flex;color:red}",
    ),
    false,
  );
  assert.equal(
    hasEquivalentDeclarationOrder("@media (width > 1px){.button{color:red}}", ".button{color:red}"),
    false,
  );
});
