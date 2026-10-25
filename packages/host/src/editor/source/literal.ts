import type { NativeCandidateContext } from "@stillwind/native";
import type { PreparedSource } from "@stillwind/projection";
import ts from "typescript-6";

import type { PreviewSite } from "../previews.ts";

/**
 * A literal in a compiler-recognized style input, with decoded-to-source boundaries.
 */
export interface UtilityLiteral {
  /** Prepared source whose original buffer owns all recorded boundaries. */
  source: PreparedSource;
  /** Literal token in the source's checker AST, which may use projected text. */
  node: ts.StringLiteralLike | ts.TemplateLiteralToken;
  /** Decoded literal content without delimiters or enclosing group variants. */
  text: string;
  /** Variants supplied by surrounding named style groups, including trailing colons. */
  prefix: string;
  /** Input role that selects completion registries, validation, and permitted variants. */
  context: NativeCandidateContext;
  /**
   * Captured occurrence whose native plan retains original UTF-8 ranges.
   * Syntactically recognized incomplete input can have a context without a site.
   */
  site?: PreviewSite;
  /**
   * Original UTF-16 boundaries for every position in `text`, including its end.
   * The array has `text.length + 1` entries. One escape that emits several code units
   * can give several decoded positions the same source boundary.
   */
  boundaries: readonly number[];
}

/**
 * Decodes JS literal content while mapping every decoded position to its source spelling.
 * CR and CRLF become LF. Escaped line continuations emit no text and move the preceding
 * boundary past the continuation. The caller must compare the result with the AST's decoded text.
 *
 * @param raw - Literal content without its opening delimiter or closing delimiter/interpolation.
 * @param start - Original UTF-16 offset of the first code unit in `raw`.
 * @returns Decoded text and `text.length + 1` source boundaries, or undefined for a trailing
 * backslash, invalid hex digits, a missing Unicode brace, or a code point above U+10FFFF.
 */
export function decodeLiteral(
  raw: string,
  start: number,
):
  | {
      /** Decoded content. Literal line endings are normalized to LF. */
      text: string;
      /** Original UTF-16 offsets for every decoded position, including the end. */
      boundaries: readonly number[];
    }
  | undefined {
  let text = "";
  const boundaries = [start];
  for (let index = 0; index < raw.length;) {
    const begin = index;
    let value = raw[index++];
    if (value === "\\") {
      const escape = raw[index++];
      if (escape === undefined) {
        return undefined;
      }
      if (escape === "\n" || escape === "\r") {
        if (escape === "\r" && raw[index] === "\n") {
          index++;
        }
        boundaries[boundaries.length - 1] = start + index;
        continue;
      }
      if (escape === "x" || escape === "u") {
        const brace = escape === "u" && raw[index] === "{";
        const end = brace ? raw.indexOf("}", index + 1) : index + (escape === "x" ? 2 : 4);
        const digits = raw.slice(index + (brace ? 1 : 0), end);
        if (end < index || !/^[\da-f]+$/i.test(digits)) {
          return undefined;
        }
        const point = Number.parseInt(digits, 16);
        if (point > 0x10ffff) {
          return undefined;
        }
        value = String.fromCodePoint(point);
        index = end + (brace ? 1 : 0);
      } else {
        const escapes: Record<string, string> = {
          n: "\n",
          r: "\r",
          t: "\t",
          b: "\b",
          f: "\f",
          v: "\v",
          "0": "\0",
        };
        value = escapes[escape] ?? escape;
      }
    } else if (value === "\r") {
      if (raw[index] === "\n") {
        index++;
      }
      value = "\n";
    }
    text += value;
    for (let unit = 1; unit <= value.length; unit++) {
      boundaries.push(start + (unit === value.length ? index : begin));
    }
  }
  return {
    text,
    boundaries,
  };
}
