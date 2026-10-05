/**
 * Converts a dynamic value into text for a generated CSS declaration.
 *
 * @param value - CSS text, a number, a URL, or a boolean. URLs are quoted and CSS-escaped. True
 * becomes a space and false becomes `initial`.
 * @returns CSS value text. Strings and numbers are converted with `String`.
 */
export function stillwindCssValue(value: URL | string | number | boolean): string {
  if (typeof value === "boolean") {
    return value ? " " : "initial";
  }
  if (value instanceof URL) {
    const escaped = value.href.replace(
      /["\\\n\r\f]/g,
      (character) => `\\${character.codePointAt(0)!.toString(16)} `,
    );
    return `url("${escaped}")`;
  }
  return String(value);
}

/**
 * Flattens class contributions and joins nonempty values with spaces.
 *
 * @param values - Strings, numbers, bigints, nested arrays, or objects whose truthy values enable
 * their keys. Other values are ignored.
 * @returns Class text in contribution order, without deduplication.
 */
export function stillwindClsx(...values: readonly unknown[]): string {
  return values.flatMap(flatten).filter(Boolean).join(" ");
}

/**
 * Checks flattened class contributions before joining them with spaces.
 *
 * @param values - Contributions accepted by `stillwindClsx`. Flattening converts numeric values to
 * strings.
 * @returns The same class text as `stillwindClsx` when all flattened values are strings.
 * @throws If flattening produces a non-string value.
 */
export function stillwindClsxChecked(...values: readonly unknown[]): string {
  for (const value of values.flatMap(flatten)) {
    if (typeof value !== "string") {
      throw new TypeError(`Stillwind received a non-string class value: ${typeof value}.`);
    }
  }
  return stillwindClsx(...values);
}

/**
 * Escapes text for use as one CSS identifier.
 *
 * @param value - Unescaped identifier text.
 * @returns `CSS.escape` output when available, otherwise escaped text with NUL replaced by U+FFFD.
 */
export function stillwindEscapedIdentifier(value: string): string {
  if (typeof CSS !== "undefined" && typeof CSS.escape === "function") {
    return CSS.escape(value);
  }
  const characters = Array.from(value);
  let escaped = "";
  for (const [index, character] of characters.entries()) {
    const code = character.codePointAt(0)!;
    const isDigit = code >= 48 && code <= 57;
    const isControl = (code >= 1 && code <= 31) || code === 127;
    if (code === 0) {
      escaped += "\uFFFD";
    } else if (isControl) {
      escaped += `\\${code.toString(16)} `;
    } else if (index === 0 && isDigit) {
      escaped += `\\${code.toString(16)} `;
    } else if (index === 0 && character === "-" && characters.length === 1) {
      escaped += "\\-";
    } else if (index === 1 && isDigit && characters[0] === "-") {
      escaped += `\\${code.toString(16)} `;
    } else if (
      isDigit ||
      (code >= 65 && code <= 90) ||
      (code >= 97 && code <= 122) ||
      code === 45 ||
      code === 95 ||
      code >= 128
    ) {
      escaped += character;
    } else {
      escaped += `\\${character}`;
    }
  }
  return escaped;
}

/**
 * Builds a CSS identifier for one keyed view transition.
 *
 * @param value - Nonempty class token or an object whose `v` property contains that token.
 * @param key - Key appended to the token after a hyphen using `String`.
 * @returns The combined token and key escaped as a CSS identifier.
 * @throws If the value does not supply a nonempty string token.
 */
export function stillwindViewTransitionName(value: unknown, key: string | number): string {
  const record =
    value && typeof value === "object" && "v" in value ? (value as { v?: unknown }) : undefined;
  const token = typeof value === "string" ? value : record?.v;
  if (typeof token !== "string" || token.length === 0) {
    throw new TypeError("Stillwind received a view transition without a class token.");
  }
  return stillwindEscapedIdentifier(`${token}-${String(key)}`);
}

/**
 * Expands one class contribution into strings without splitting class text.
 *
 * @param value - Class text, a number, a bigint, a nested array, or an object with enabled class
 * keys.
 * @returns Strings in traversal or object-entry order. Unsupported values produce an empty list.
 */
function flatten(value: unknown): string[] {
  if (typeof value === "string" || typeof value === "number" || typeof value === "bigint") {
    return [String(value)];
  }
  if (Array.isArray(value)) {
    return value.flatMap(flatten);
  }
  if (value && typeof value === "object") {
    return Object.entries(value as Record<string, unknown>)
      .filter(([, enabled]) => Boolean(enabled))
      .map(([name]) => name);
  }
  return [];
}
