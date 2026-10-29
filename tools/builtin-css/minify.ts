import { tokenize, tokenTypes } from "css-tree";

/**
 * Removes comments and formatting whitespace without rewriting Stillwind definitions.
 */
export function minifyCss(source: string): string {
  const input: {
    type: number;
    value: string;
  }[] = [];
  tokenize(source, (type, start, end) => {
    input.push({
      type,
      value: source.slice(start, end),
    });
  });
  const tokens: typeof input = [];
  for (let index = 0; index < input.length; index++) {
    const token = input[index];
    if (token.type === tokenTypes.Comment) {
      while (input[index + 1]?.type === tokenTypes.Comment) {
        index++;
      }
      if (
        tokens.length === 0 ||
        index + 1 === input.length ||
        tokens.at(-1)?.type === tokenTypes.WhiteSpace ||
        input[index + 1].type === tokenTypes.WhiteSpace
      ) {
        continue;
      }
      // An empty comment keeps adjacent tokens separate without adding a descendant combinator.
      tokens.push({
        type: tokenTypes.Comment,
        value: "/**/",
      });
    } else if (
      token.type !== tokenTypes.WhiteSpace ||
      tokens.at(-1)?.type !== tokenTypes.WhiteSpace
    ) {
      tokens.push(token);
    }
  }
  const boundary = new Set<number>([
    tokenTypes.LeftCurlyBracket,
    tokenTypes.RightCurlyBracket,
    tokenTypes.Semicolon,
  ]);
  let output = "";
  for (let index = 0; index < tokens.length; index++) {
    const token = tokens[index];
    const before = tokens[index - 1];
    const after = tokens[index + 1];
    if (token.type === tokenTypes.WhiteSpace) {
      const emptyValue =
        before?.type === tokenTypes.Colon &&
        (after?.type === tokenTypes.Semicolon || after?.type === tokenTypes.RightCurlyBracket);
      if (
        before &&
        after &&
        (emptyValue || (!boundary.has(before.type) && !boundary.has(after.type)))
      ) {
        output += " ";
      }
    } else {
      output += token.value;
    }
  }
  return output.trim();
}
