import { mergeFragments, type FragmentRuntimeOptions } from "./fragment.ts";
import { stillwindClsx } from "./shared.ts";

/** Compiler-produced class text, fragment channels, or nested fragment arrays. */
export type CvaValue = string | boolean | null | undefined | object;

/** Compiled configuration whose structure is immutable after factory creation. */
export interface CompiledCvaConfig {
  readonly variants?: Readonly<Record<string, Readonly<Record<string, CvaValue>>>>;
  readonly defaultVariants?: Readonly<Record<string, string | number | boolean | null | undefined>>;
  readonly compoundVariants?: readonly Readonly<Record<string, unknown>>[];
}

function optionKey(value: unknown): unknown {
  return typeof value === "boolean" ? String(value) : value === 0 ? "0" : value;
}

/**
 * Creates a selector over compiler-produced values using the supplied framework channels.
 * Values are captured at creation. Each call returns fresh props without mutating inputs.
 * Undefined selections keep defaults; null suppresses an option. Matching compounds
 * contribute in array order, followed by external class and className inputs.
 *
 * @param options - Framework conventions and the keys used by compiled fragments.
 * @param base - Compiled styles included in every selection.
 * @param config - Immutable compiled option maps, defaults, and compound conditions.
 * @returns A selector that materializes class and optional style props without private symbols.
 */
export function createCva(
  options: FragmentRuntimeOptions,
  base: CvaValue,
  config: CompiledCvaConfig = {},
): (props?: Readonly<Record<string, unknown>>) => Record<string, unknown> {
  const axes = Object.keys(config.variants ?? {});
  const compounds = (config.compoundVariants ?? []).map((compound) => ({
    conditions: Object.entries(compound).filter(([key]) => key !== "class" && key !== "className"),
    class: compound.class as CvaValue,
    className: compound.className as CvaValue,
  }));
  const fragment = (value: CvaValue): unknown =>
    typeof value === "string" ? { [options[2]]: value } : value;

  return (props = {}) => {
    const values: unknown[] = [fragment(base)];
    const variants = config.variants;
    if (variants) {
      for (const axis of axes) {
        const supplied = props[axis];
        if (supplied === null) {
          continue;
        }
        const key = optionKey(supplied) || optionKey(config.defaultVariants?.[axis]);
        const choices = variants[axis]!;
        if ((typeof key === "string" || typeof key === "number") && Object.hasOwn(choices, key)) {
          values.push(fragment(choices[key]));
        }
      }
      const resolved = {
        ...config.defaultVariants,
        ...Object.fromEntries(Object.entries(props).filter(([, value]) => value !== undefined)),
      };
      for (const compound of compounds) {
        if (
          compound.conditions.every(([key, expected]) =>
            Array.isArray(expected) ? expected.includes(resolved[key]) : expected === resolved[key],
          )
        ) {
          values.push(fragment(compound.class), fragment(compound.className));
        }
      }
    }
    values.push({ [options[2]]: stillwindClsx(props.class, props.className) });
    return mergeFragments(options, values);
  };
}
