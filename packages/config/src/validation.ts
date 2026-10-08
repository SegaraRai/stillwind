/** Field validation shared by compiler settings and lint presets. */

export function text(value: unknown): value is string {
  return typeof value === "string" && Boolean(value.trim());
}

export function requireValue(
  condition: unknown,
  location: string,
  message: string,
): asserts condition {
  if (!condition) {
    throw new Error(`${location}: ${message}`);
  }
}

export function record(value: unknown, location: string): asserts value is Record<string, unknown> {
  requireValue(
    value !== null && typeof value === "object" && !Array.isArray(value),
    location,
    "must be an object",
  );
}

export function keys(value: Record<string, unknown>, allowed: readonly string[], location: string) {
  for (const key of Object.keys(value)) {
    requireValue(allowed.includes(key), `${location}.${key}`, "unknown setting");
  }
}

export function stringArray(
  value: unknown,
  location: string,
  nonempty = false,
): asserts value is string[] {
  requireValue(
    Array.isArray(value) && (!nonempty || value.length > 0) && value.every(text),
    location,
    "must be an array of nonempty strings",
  );
}

export function object(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
