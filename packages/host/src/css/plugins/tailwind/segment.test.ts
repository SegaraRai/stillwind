import { expect, test } from "vite-plus/test";

import { isValidArbitrary, segment } from "./segment.ts";

test("segments split outside strings and groups that only their own closer ends", () => {
  expect(segment("a,b(c,d),[e,f],{g,h},'i,j',\"k,l\",m\\,n", ",")).toEqual([
    "a",
    "b(c,d)",
    "[e,f]",
    "{g,h}",
    "'i,j'",
    '"k,l"',
    "m\\,n",
  ]);
  expect(segment("(a],b),c", ",")).toEqual(["(a],b)", "c"]);
  expect(segment("a)/b", "/")).toEqual(["a)", "b"]);
  expect(segment("", ",")).toEqual([""]);
});

test("arbitrary values are rejected for top-level semicolons and unbalanced groups", () => {
  for (const value of ["1px", "(a})", "url('a;b')", "f(a;b)", "a\\;b"]) {
    expect(isValidArbitrary(value), value).toBe(true);
  }
  for (const value of ["a;b", "a{b}", "a)", "(a]", "(a", "[a"]) {
    expect(isValidArbitrary(value), value).toBe(false);
  }
});
