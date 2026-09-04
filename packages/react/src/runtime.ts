/** A deliberately small clsx-compatible helper used only for static branches. */
export function stillwindClsx(...values: readonly unknown[]): string {
  return values.flatMap(flatten).filter(Boolean).join(" ");
}

export interface StillwindRuntimeHandle {
  readonly kind: "ref" | "scope" | "projection";
  readonly className: string;
}

interface StillwindRuntimeAsset {
  readonly kind: "animation" | "view-transition" | "animation-use";
  readonly identity: string;
}

/** Creates a callable animation identity for TypeScript-selected runtime branches. */
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

/** Builds the shared custom-property payload for every view-transition use. */
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

/** Resolves a generated class without accepting arbitrary objects as compiler assets. */
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

/** Creates the minimal runtime value needed when a ref crosses static data flow. */
export function stillwindRef(className: string): StillwindRuntimeHandle & {
  readonly anchor: StillwindRuntimeHandle;
  readonly position: StillwindRuntimeHandle;
  readonly anchorScope: StillwindRuntimeHandle;
  container(type: "size" | "inline-size"): StillwindRuntimeHandle;
} {
  const projection = (name: string): StillwindRuntimeHandle => ({
    kind: "projection",
    className: `_swp_${className}_${name}`,
  });
  return {
    kind: "ref",
    className,
    anchor: projection("anchor"),
    position: projection("position"),
    anchorScope: projection("scope"),
    container: projection,
  };
}

/** Creates the minimal runtime value needed when a scope crosses static data flow. */
export function stillwindScope(className: string): StillwindRuntimeHandle {
  return { kind: "scope", className };
}

/** Reads a marker without accepting arbitrary user objects as Stillwind handles. */
export function stillwindHandleClass(value: unknown): string {
  if (
    !value ||
    typeof value !== "object" ||
    !("kind" in value) ||
    (value.kind !== "ref" && value.kind !== "scope" && value.kind !== "projection") ||
    !("className" in value) ||
    typeof value.className !== "string"
  ) {
    throw new TypeError("Stillwind expected a compiled ref or scope value.");
  }
  return value.className;
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
