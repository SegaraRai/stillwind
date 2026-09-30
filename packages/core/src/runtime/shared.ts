/**
 * Converts inline custom-property values to CSS text. Strings remain authored CSS,
 * numbers have no unit, and URL objects become url(). Boolean true becomes a single
 * space (an empty CSS value that CSSOM does not remove); false becomes initial.
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

/** A deliberately small clsx-compatible helper used only for static branches. */
export function stillwindClsx(...values: readonly unknown[]): string {
  return values.flatMap(flatten).filter(Boolean).join(" ");
}

/**
 * The development variant of {@link stillwindClsx}: a value that cannot become part of
 * a class attribute is reported instead of being dropped silently. The compiler imports
 * this helper only in development mode.
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
 * Serializes one CSS identifier, matching `CSS.escape` and CSS Syntax "serialize an
 * identifier". A value that is already an identifier is returned unchanged, so the
 * compiled names and the names this function composes agree byte for byte.
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
 * Returns a participant's `view-transition-name` by joining its class token and key
 * with `-` and escaping the result for use as a CSS value.
 *
 * `value` must be a nonempty token string or a compiled record with that string in `v`.
 * `key` is the string or number passed by the caller. It is converted with String before escaping.
 * Throws TypeError when the token is missing, empty, or not a string.
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
