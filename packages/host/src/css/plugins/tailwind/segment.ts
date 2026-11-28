// Ported from Tailwind CSS 4.3.5 (MIT), utils/segment.ts and utils/is-valid-arbitrary.ts. Keep
// the scanning rules aligned with upstream: Tailwind splits plugin selectors, candidate parts, and
// arbitrary values with them, so a CSS tokenizer would accept or split text that Tailwind does not.

/**
 * Splits text at a separator outside quotes and bracket groups, as Tailwind's `segment()` does.
 *
 * A backslash escapes the next character. A `'` or `"` starts a string that ends at the same
 * quote, and backslashes escape characters inside it. `(`, `[`, and `{` open groups that only
 * their own closer ends. A closer that does not match the innermost open group is ignored.
 *
 * @param input - Text to split.
 * @param separator - One character, such as `,`, `/`, or ` `. Its first UTF-16 code unit is
 * compared.
 * @returns The parts without trimming, including empty ones. Text without the separator is one
 * part.
 */
export function segment(input: string, separator: string): string[] {
  const closers: number[] = [];
  const parts: string[] = [];
  const delimiter = separator.charCodeAt(0);
  let start = 0;
  for (let index = 0; index < input.length; index++) {
    const code = input.charCodeAt(index);
    if (closers.length === 0 && code === delimiter) {
      parts.push(input.slice(start, index));
      start = index + 1;
      continue;
    }
    index = skip(input, index, code, closers);
  }
  parts.push(input.slice(start));
  return parts;
}

/**
 * Whether an arbitrary value is usable, as Tailwind's `isValidArbitrary()` decides.
 *
 * The value is rejected when it has a `;` outside groups, a `)` or `]` that does not close the
 * innermost group, a closer without an open group, or an unclosed `(` or `[`. A `{` opens no group,
 * so a `}` is rejected outside groups and ignored inside them.
 *
 * @param input - Decoded arbitrary value.
 * @returns Whether Tailwind accepts the value.
 */
export function isValidArbitrary(input: string): boolean {
  const closers: number[] = [];
  for (let index = 0; index < input.length; index++) {
    const code = input.charCodeAt(index);
    switch (code) {
      case BACKSLASH:
      case SINGLE_QUOTE:
      case DOUBLE_QUOTE:
        index = skip(input, index, code, closers);
        break;

      case OPEN_PAREN:
        closers.push(CLOSE_PAREN);
        break;

      case OPEN_BRACKET:
        closers.push(CLOSE_BRACKET);
        break;

      case CLOSE_BRACKET:
      case CLOSE_CURLY:
      case CLOSE_PAREN:
        if (closers.length === 0) {
          return false;
        }
        if (code === closers.at(-1)) {
          closers.pop();
        } else if (code !== CLOSE_CURLY) {
          return false;
        }
        break;

      case SEMICOLON:
        if (closers.length === 0) {
          return false;
        }
        break;
    }
  }
  return closers.length === 0;
}

const BACKSLASH = 0x5c;
const SINGLE_QUOTE = 0x27;
const DOUBLE_QUOTE = 0x22;
const OPEN_PAREN = 0x28;
const CLOSE_PAREN = 0x29;
const OPEN_BRACKET = 0x5b;
const CLOSE_BRACKET = 0x5d;
const OPEN_CURLY = 0x7b;
const CLOSE_CURLY = 0x7d;
const SEMICOLON = 0x3b;

/**
 * Reads one character of {@link segment} outside a string.
 *
 * @param input - Scanned text.
 * @param index - Index of the character.
 * @param code - UTF-16 code unit at `index`.
 * @param closers - Closers of the open groups, innermost last. A group opener pushes its closer,
 * and a matching closer pops it.
 * @returns The index of the last character that the call consumed: after an escape, the escaped
 * character, and after a quote, its closing quote or the last character of `input`.
 */
function skip(input: string, index: number, code: number, closers: number[]): number {
  switch (code) {
    case BACKSLASH:
      return index + 1;

    case SINGLE_QUOTE:
    case DOUBLE_QUOTE:
      while (++index < input.length) {
        const next = input.charCodeAt(index);
        if (next === BACKSLASH) {
          index += 1;
          continue;
        }
        if (next === code) {
          break;
        }
      }
      return index;

    case OPEN_PAREN:
      closers.push(CLOSE_PAREN);
      return index;

    case OPEN_BRACKET:
      closers.push(CLOSE_BRACKET);
      return index;

    case OPEN_CURLY:
      closers.push(CLOSE_CURLY);
      return index;

    case CLOSE_BRACKET:
    case CLOSE_CURLY:
    case CLOSE_PAREN:
      if (closers.length > 0 && code === closers.at(-1)) {
        closers.pop();
      }
      return index;

    default:
      return index;
  }
}
