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

/** Creates a view-transition identity for TypeScript-selected runtime branches. */
export function stillwindViewTransition(identity: string): StillwindRuntimeAsset & {
  for(key: string | number): StillwindRuntimeAsset & { readonly key: string | number };
} {
  return {
    kind: "view-transition",
    identity,
    for: (key) => ({ kind: "view-transition", identity, key }),
  };
}

/**
 * Returns name/class custom properties for an identity in candidates. value may
 * be the identity string or a view-transition record. An explicit key takes
 * precedence over the record key; String(key) is encoded as hyphen-separated
 * hexadecimal Unicode code points after the identity and `-k-` suffix.
 * The caller supplies distinct CSS custom-property names. Throws TypeError for
 * an unsupported value or identity outside candidates.
 */
export function stillwindViewTransitionStyle(
  value: unknown,
  key: string | number | undefined,
  candidates: readonly string[],
  nameProperty: string,
  classProperty: string,
): Record<string, string> {
  const runtimeValue =
    typeof value === "string"
      ? undefined
      : value &&
          typeof value === "object" &&
          "kind" in value &&
          value.kind === "view-transition" &&
          "identity" in value &&
          typeof value.identity === "string"
        ? value
        : undefined;
  const identity = typeof value === "string" ? value : runtimeValue?.identity;
  if (typeof identity !== "string" || !candidates.includes(identity)) {
    throw new TypeError("Stillwind received an unknown view transition identity.");
  }
  const runtimeKey =
    key ??
    (runtimeValue &&
    "key" in runtimeValue &&
    (typeof runtimeValue.key === "string" || typeof runtimeValue.key === "number")
      ? runtimeValue.key
      : undefined);
  const name =
    runtimeKey === undefined
      ? identity
      : `${identity}-k-${Array.from(String(runtimeKey), (character) =>
          character.codePointAt(0)!.toString(16),
        ).join("-")}`;
  return { [nameProperty]: name, [classProperty]: identity };
}

/**
 * Looks up an object or function's string identity in the own entries of classes.
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
  const identity = "identity" in value ? value.identity : undefined;
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
