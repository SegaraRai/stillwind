import { mergeFragments, type FragmentRuntimeOptions } from "./fragment.ts";
import { stillwindClsx } from "./shared.ts";

/**
 * Compiler-produced class text, fragment channels, or nested fragment arrays.
 */
export type RecipeValue = string | boolean | null | undefined | object;

/**
 * Compiled configuration whose structure is immutable after factory creation.
 */
export interface CompiledRecipeConfig {
  /**
   * Values of named element parts. With slots, a variant option, compound `class`, or compound
   * `className` value may be an object whose string keys name slots. Any other value applies to
   * the `base` slot. Compiled values have no string keys, so they are never read as slot maps.
   */
  readonly slots?: Readonly<Record<string, RecipeValue>>;
  readonly variants?: Readonly<Record<string, Readonly<Record<string, RecipeValue>>>>;
  readonly defaultVariants?: Readonly<Record<string, string | number | boolean | null | undefined>>;
  readonly compoundVariants?: readonly Readonly<Record<string, unknown>>[];
  /** Conditions that add `class` and `className` to every slot listed in `slots`. */
  readonly compoundSlots?: readonly Readonly<Record<string, unknown>>[];
}

/** Returns props for one element from variant choices, external classes, and a forwarded value. */
export type RecipeSelector = (
  props?: Readonly<Record<string, unknown>>,
  forwarded?: unknown,
) => Record<string, unknown>;

function optionKey(value: unknown): unknown {
  return typeof value === "boolean" ? String(value) : value === 0 ? "0" : value;
}

function isSlotMap(value: unknown): value is Readonly<Record<string, RecipeValue>> {
  return (
    typeof value === "object" &&
    value !== null &&
    !Array.isArray(value) &&
    Object.keys(value).length > 0
  );
}

function compound(entry: Readonly<Record<string, unknown>>, reserved: readonly string[]) {
  return {
    conditions: Object.entries(entry).filter(([key]) => !reserved.includes(key)),
    class: entry.class as RecipeValue,
    className: entry.className as RecipeValue,
    slots: entry.slots as readonly string[] | undefined,
  };
}

/**
 * Creates a selector over compiler-produced values using the supplied framework channels.
 * Values are captured at creation. Each call returns fresh props without mutating inputs.
 * Undefined selections keep defaults; null suppresses an option. Matching compounds
 * contribute in array order, followed by external class and className inputs. The optional
 * second argument is a fragment that `style.forward()` produced. It applies after every other
 * contribution.
 *
 * With `config.slots`, the selector returns an object with one selector for each slot and for
 * `base`. The `base` value and the classes and forwarded value of the outer call apply to the
 * `base` slot only. A slot selector accepts variant choices that replace the outer choices, and
 * its own external classes and forwarded value. A slot receives, in order: `base` (for the `base`
 * slot) and its slot value, its value of each selected option, the matching compound variants,
 * and the matching compound slots that list it.
 *
 * @param options - Framework conventions and the keys used by compiled fragments.
 * @param base - Compiled styles included in every selection, or in the `base` slot.
 * @param config - Immutable compiled slots, option maps, defaults, and compound conditions.
 * @param classes - Concatenates external class inputs. Development calls supply validation.
 * @param merge - Materializes selected fragments. Development calls supply validation.
 * @returns A selector that materializes class and optional style props without private symbols,
 * or a selector of slot selectors when `config.slots` is present.
 */
export function createRecipe(
  options: FragmentRuntimeOptions,
  base: RecipeValue,
  config: CompiledRecipeConfig = {},
  classes: typeof stillwindClsx = stillwindClsx,
  merge: typeof mergeFragments = mergeFragments,
): RecipeSelector {
  const axes = Object.keys(config.variants ?? {});
  const compounds = (config.compoundVariants ?? []).map((entry) =>
    compound(entry, ["class", "className"]),
  );
  const compoundSlots = (config.compoundSlots ?? []).map((entry) =>
    compound(entry, ["class", "className", "slots"]),
  );
  const slots = config.slots;
  const fragment = (value: RecipeValue): unknown =>
    typeof value === "string" ? { [options[2]]: value } : value;
  // Without slots, `slot` is undefined and every value applies as written.
  const pick = (value: RecipeValue, slot: string | undefined): unknown =>
    fragment(
      slot === undefined
        ? value
        : isSlotMap(value)
          ? Object.hasOwn(value, slot)
            ? value[slot]
            : undefined
          : slot === "base"
            ? value
            : undefined,
    );
  const matches = (
    conditions: readonly [string, unknown][],
    resolved: Readonly<Record<string, unknown>>,
  ) =>
    conditions.every(([key, expected]) =>
      Array.isArray(expected) ? expected.includes(resolved[key]) : expected === resolved[key],
    );

  const select = (props: Readonly<Record<string, unknown>>, slot: string | undefined) => {
    const values: unknown[] = [];
    if (slot === undefined || slot === "base") {
      values.push(fragment(base));
    }
    if (slot !== undefined && slots && Object.hasOwn(slots, slot)) {
      values.push(fragment(slots[slot]));
    }
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
          values.push(pick(choices[key], slot));
        }
      }
    }
    const resolved = {
      ...config.defaultVariants,
      ...Object.fromEntries(Object.entries(props).filter(([, value]) => value !== undefined)),
    };
    if (variants) {
      for (const entry of compounds) {
        if (matches(entry.conditions, resolved)) {
          values.push(pick(entry.class, slot), pick(entry.className, slot));
        }
      }
    }
    if (slot !== undefined) {
      for (const entry of compoundSlots) {
        if (entry.slots?.includes(slot) && matches(entry.conditions, resolved)) {
          values.push(fragment(entry.class), fragment(entry.className));
        }
      }
    }
    return values;
  };

  if (!slots) {
    return (props = {}, forwarded) =>
      merge(options, [
        ...select(props, undefined),
        {
          [options[2]]: classes(props.class, props.className),
        },
        forwarded,
      ]);
  }
  const names = ["base", ...Object.keys(slots).filter((name) => name !== "base")];
  return (props = {}, forwarded) =>
    Object.fromEntries(
      names.map((slot): [string, RecipeSelector] => [
        slot,
        (slotProps = {}, slotForwarded) => {
          const values = select(
            {
              ...props,
              ...Object.fromEntries(
                Object.entries(slotProps).filter(([, value]) => value !== undefined),
              ),
            },
            slot,
          );
          if (slot === "base") {
            values.push(
              {
                [options[2]]: classes(props.class, props.className),
              },
              forwarded,
            );
          }
          values.push(
            {
              [options[2]]: classes(slotProps.class, slotProps.className),
            },
            slotForwarded,
          );
          return merge(options, values);
        },
      ]),
    );
}
