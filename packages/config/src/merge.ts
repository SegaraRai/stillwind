import { object } from "./validation.ts";

/**
 * Merges objects recursively. Undefined preserves previous values, arrays are replaced, and a
 * changed provider kind replaces that provider's complete configuration.
 *
 * @param base - Settings to preserve where overrides omit a value.
 * @param overrides - Fields to merge or replace. Neither input is modified.
 * @returns Merged settings. Unchanged values and replacement arrays can share input references.
 */
export function mergeConfig<T extends object>(base: T, overrides: Partial<T>): T {
  return mergeValue(base, overrides) as T;
}

function mergeValue(base: unknown, override: unknown): unknown {
  if (override === undefined) {
    return base;
  }
  if (
    !object(base) ||
    !object(override) ||
    (override.kind !== undefined && base.kind !== override.kind)
  ) {
    return override;
  }
  const result = { ...base };
  for (const [key, value] of Object.entries(override)) {
    if (value !== undefined) {
      result[key] = mergeValue(base[key], value);
    }
  }
  return result;
}
