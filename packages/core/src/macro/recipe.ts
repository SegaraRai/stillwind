import type { StillwindClassValue, StyleCustomPropertyValue, StyleVariant } from "./values.ts";

/** Static rule inputs accepted by variant keys and the keys `""` and `":"` in a recipe definition. */
export type RecipeRuleInput =
  | string
  | boolean
  | null
  | undefined
  | readonly RecipeRuleInput[]
  | {
      readonly ""?: RecipeRuleInput;
      readonly [variant: StyleVariant]: RecipeRuleInput;
    };

/**
 * Statically written utilities, arrays, CSS variants, and custom properties.
 * The compiler rejects `&&` and ternary expressions in utility inputs, including
 * arrays and CSS variants. TypeScript checks value types and cannot enforce these
 * syntax restrictions. Custom-property values may contain expressions.
 * Custom properties remain inline, including literal values. Stored input objects,
 * spreads, declaration uses, and composed style results are unsupported.
 */
export type RecipeInput =
  | string
  | boolean
  | null
  | undefined
  | readonly RecipeInput[]
  | {
      readonly ""?: RecipeRuleInput;
      readonly [variant: StyleVariant]: RecipeRuleInput;
      readonly [property: `--${string}`]: StyleCustomPropertyValue;
    };

/** Finite option maps written directly in a recipe configuration. */
export type RecipeVariants = Readonly<Record<string, Readonly<Record<string, RecipeInput>>>>;

type Option<Key> = Key extends "true" | "false" ? boolean : Key;

/**
 * Option maps of a recipe with slots. The compiler checks each option value: an object whose keys
 * all name slots selects inputs by slot, and any other input applies to the `base` slot.
 */
export type RecipeSlotVariants = Readonly<Record<string, Readonly<Record<string, unknown>>>>;

/** Option maps whose values are inputs for the `base` slot or inputs keyed by slot name. */
export type RecipeSlotVariantsOf<Slots extends RecipeSlots> = Readonly<
  Record<string, Readonly<Record<string, RecipeSlotInput<Slots>>>>
>;

/** Named element parts of a recipe and the inputs that every selection adds to them. */
export type RecipeSlots = Readonly<Record<string, RecipeInput>>;

/** A slot of a recipe, including the `base` slot that every recipe with slots has. */
export type RecipeSlotName<Slots extends RecipeSlots> = (keyof Slots & string) | "base";

/** An input for the `base` slot, or inputs keyed by slot name. */
export type RecipeSlotInput<Slots extends RecipeSlots> =
  | RecipeInput
  | {
      readonly [Slot in RecipeSlotName<Slots>]?: RecipeInput;
    };

/** Variant choices; undefined keeps the default, and null suppresses the option. */
export type RecipeSelection<Variants extends RecipeSlotVariants> = {
  readonly [Key in keyof Variants]?: Option<keyof Variants[Key]> | null | undefined;
};

/** External classes appended after selected options and compound variants. */
export type RecipeClassProps = {
  readonly class?: StillwindClassValue;
  readonly className?: StillwindClassValue;
};

/** Option conditions of a compound entry. An array matches any of its options. */
export type RecipeConditions<Variants extends RecipeSlotVariants> = {
  readonly [Key in keyof Variants]?:
    | Option<keyof Variants[Key]>
    | null
    | undefined
    | readonly (Option<keyof Variants[Key]> | null | undefined)[];
};

/** A conjunction of option conditions and the styles added when all conditions match. */
export type RecipeCompound<Variants extends RecipeVariants> = RecipeConditions<Variants> & {
  readonly class?: RecipeInput;
  readonly className?: RecipeInput;
};

/** Static configuration for a recipe factory. The class prop names cannot be variant axes. */
export type RecipeConfig<Variants extends RecipeVariants> = {
  readonly variants?: Variants & {
    readonly class?: never;
    readonly className?: never;
  };
  readonly defaultVariants?: RecipeSelection<Variants>;
  readonly compoundVariants?: readonly RecipeCompound<Variants>[];
};

/** Framework props for an element; recipe results cannot be composed through style(). */
export type RecipeResult<Props> = Pick<
  Props,
  Extract<keyof Props, "class" | "className" | "style">
>;

/** Extracts variant choices from a recipe factory, excluding external class inputs. */
export type VariantProps<Factory extends (...args: never[]) => unknown> = Omit<
  Exclude<Parameters<Factory>[0], undefined>,
  "class" | "className"
>;

/**
 * Static configuration for a recipe factory with slots. Option values and compound class inputs
 * may select inputs by slot. `compoundSlots` entries add their class inputs to every listed slot
 * when all conditions match.
 */
export type RecipeSlotConfig<
  Slots extends RecipeSlots,
  Variants extends RecipeSlotVariantsOf<Slots>,
> = {
  readonly slots: Slots;
  readonly variants?: Variants & { readonly class?: never; readonly className?: never };
  readonly defaultVariants?: RecipeSelection<Variants>;
  readonly compoundVariants?: readonly (RecipeConditions<Variants> & {
    readonly class?: RecipeSlotInput<Slots>;
    readonly className?: RecipeSlotInput<Slots>;
  })[];
  readonly compoundSlots?: readonly (RecipeConditions<Variants> & {
    readonly slots: readonly RecipeSlotName<Slots>[];
    readonly class?: RecipeInput;
    readonly className?: RecipeInput;
  })[];
};
