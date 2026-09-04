/** A deliberately small clsx-compatible helper used only for static branches. */
export function stillwindClsx(...values: readonly unknown[]): string {
  return values.flatMap(flatten).filter(Boolean).join(" ");
}

export interface StillwindRuntimeHandle {
  readonly kind: "ref" | "scope";
  readonly className: string;
}

/** Creates the minimal runtime value needed when a ref crosses static data flow. */
export function stillwindRef(className: string): StillwindRuntimeHandle {
  return { kind: "ref", className };
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
    (value.kind !== "ref" && value.kind !== "scope") ||
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
