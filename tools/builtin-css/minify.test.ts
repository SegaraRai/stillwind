import { expect, test } from "vite-plus/test";

import { minifyCss } from "./minify.ts";

test("preserves strings, escapes, selector whitespace, and arithmetic in Stillwind definitions", () => {
  const source = `/* Registry */
@stillwind-utility example-* {
  & .child, & :hover {
    content: "/* literal */  a ; { b }";
    --escaped: \\31 0;
    width: calc(--value([length]) + 1px);
    color: var(--color, );
  }
}`;
  expect(minifyCss(source)).toBe(
    '@stillwind-utility example-*{& .child, & :hover{content: "/* literal */  a ; { b }";' +
      "--escaped: \\31 0;width: calc(--value([length]) + 1px);color: var(--color, );}}",
  );
  expect(minifyCss(".a/**/.b { color: red; } a/**/b { color: blue; }")).toBe(
    ".a/**/.b{color: red;}a/**/b{color: blue;}",
  );
  const empty = ".a { --empty: /* empty value */ ; } /* between rules */ .b { --empty: }";
  expect(minifyCss(empty)).toBe(".a{--empty: ;}.b{--empty: }");
  expect(minifyCss(minifyCss(empty))).toBe(minifyCss(empty));
  expect(minifyCss(".a\\ /**/.b { color: red; }")).toBe(".a\\ /**/.b{color: red;}");
  expect(minifyCss("a/**//**/ b { color: red; }")).toBe("a b{color: red;}");
});
