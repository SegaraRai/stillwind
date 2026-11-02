import { style } from "stillwind/astro";

import type { SyntaxKind } from "../../lib/landing/highlight";

/** Colors each syntax token kind. An ancestor can replace every color with --syntax-override. */
export const syntaxStyles = {
  api: style("syntax-api"),
  variant: style("syntax-variant"),
  utility: style("syntax-utility"),
  identifier: style("syntax-identifier"),
  property: style("syntax-property"),
  string: style("syntax-string"),
  keyword: style("syntax-keyword"),
  punctuation: style("syntax-punctuation"),
} satisfies Record<SyntaxKind, unknown>;
