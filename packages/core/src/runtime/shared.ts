/**
 * Converts inline custom-property values to CSS text. Strings remain authored CSS,
 * numbers have no unit, and URL objects become url(). Boolean true becomes a single
 * space (an empty CSS value that CSSOM does not remove); false becomes initial.
 */
export function stillwindCssValue(value: URL | string | number | boolean): string {
  if (typeof value === "boolean") return value ? " " : "initial";
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

interface StillwindRuntimeAsset {
  readonly kind: "animation" | "view-transition" | "animation-use";
  readonly identity: string;
}

/**
 * Wraps a compiler-generated animation identity in a callable runtime value.
 * Calling it returns an animation-use record with the same identity; options are
 * ignored because their CSS is resolved by the compiler. Identity is not validated.
 */
export function stillwindAnimation(identity: string) {
  return Object.assign(
    (_options: string): StillwindRuntimeAsset => ({ kind: "animation-use", identity }),
    { kind: "animation" as const, identity },
  );
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
    if (code === 0) escaped += "\uFFFD";
    else if (isControl) escaped += `\\${code.toString(16)} `;
    else if (index === 0 && isDigit) escaped += `\\${code.toString(16)} `;
    else if (index === 0 && character === "-" && characters.length === 1) escaped += "\\-";
    else if (index === 1 && isDigit && characters[0] === "-") escaped += `\\${code.toString(16)} `;
    else if (
      isDigit ||
      (code >= 65 && code <= 90) ||
      (code >= 97 && code <= 122) ||
      code === 45 ||
      code === 95 ||
      code >= 128
    )
      escaped += character;
    else escaped += `\\${character}`;
  }
  return escaped;
}

/**
 * Returns the `view-transition-name` a participant is paired under: its class token and
 * key, joined with `-`, escaped for a CSS value position.
 *
 * The value may be the token itself or one of the records the compiler emits, whose key
 * an explicit argument overrides. A record without a key names nothing, and the empty
 * string leaves the property to whatever the element's class already declares.
 * Throws TypeError for a value that carries no token at all.
 */
export function stillwindViewTransitionName(value: unknown, key?: string | number): string {
  const record =
    value && typeof value === "object" && "v" in value
      ? (value as { v?: unknown; k?: unknown })
      : undefined;
  const token = typeof value === "string" ? value : record?.v;
  if (typeof token !== "string" || token.length === 0) {
    throw new TypeError("Stillwind received a view transition without a class token.");
  }
  const resolved =
    key ?? (typeof record?.k === "string" || typeof record?.k === "number" ? record.k : undefined);
  if (resolved === undefined) return "";
  return stillwindEscapedIdentifier(`${token}-${String(resolved)}`);
}

/**
 * Returns the value a keyed view transition carries, so a key applied where the transition
 * was declared travels with it. Throws TypeError for a value without a class token.
 */
export function stillwindViewTransitionKey(
  value: unknown,
  key: string | number,
): { readonly v: string; readonly k: string | number } {
  const token =
    typeof value === "string"
      ? value
      : value && typeof value === "object" && "v" in value && typeof value.v === "string"
        ? value.v
        : undefined;
  if (typeof token !== "string" || token.length === 0) {
    throw new TypeError("Stillwind received a view transition without a class token.");
  }
  return { v: token, k: key };
}

/**
 * Looks up an object or function's string identity in the own entries of classes.
 * A view transition carries its class token under `v`, an animation under `identity`.
 * This validates table membership, not the asset kind or its provenance.
 * Throws TypeError for an unsupported value or an identity absent from the table.
 */
export function stillwindAssetClass(
  value: unknown,
  classes: Readonly<Record<string, string>>,
): string {
  if (!value || (typeof value !== "object" && typeof value !== "function")) {
    throw new TypeError("Stillwind expected a compiled animation or view transition value.");
  }
  const identity = "v" in value ? value.v : "identity" in value ? value.identity : undefined;
  if (typeof identity !== "string" || !Object.hasOwn(classes, identity)) {
    throw new TypeError("Stillwind received an unknown animation or view transition identity.");
  }
  return classes[identity];
}

function flatten(value: unknown): string[] {
  if (typeof value === "string" || typeof value === "number" || typeof value === "bigint") {
    return [String(value)];
  }
  if (Array.isArray(value)) return value.flatMap(flatten);
  if (value && typeof value === "object") {
    return Object.entries(value as Record<string, unknown>)
      .filter(([, enabled]) => Boolean(enabled))
      .map(([name]) => name);
  }
  return [];
}
