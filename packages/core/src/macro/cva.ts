import type { StillwindClassValue, StyleCustomPropertyValue, StyleVariant } from "./index.ts";

/** Rule inputs accepted by CSS variant wrappers in a CVA definition. */
export type CvaRuleInput =
  | string
  | boolean
  | null
  | undefined
  | readonly CvaRuleInput[]
  | { readonly [variant: StyleVariant]: CvaRuleInput };

/**
 * Statically written utilities, arrays, branches, CSS variants, and custom properties.
 * Custom properties remain inline, including literal values. Stored input objects,
 * spreads, declaration uses, attributes, and composed style results are unsupported.
 */
export type CvaInput =
  | string
  | boolean
  | null
  | undefined
  | readonly CvaInput[]
  | {
      readonly [variant: StyleVariant]: CvaRuleInput;
      readonly [property: `--${string}`]: StyleCustomPropertyValue;
    };

/** Finite option maps written directly in a CVA configuration. */
export type CvaVariants = Readonly<Record<string, Readonly<Record<string, CvaInput>>>>;

type Option<Key> = Key extends "true" | "false" ? boolean : Key;

/** Variant choices; undefined keeps the default, and null suppresses the option. */
export type CvaSelection<Variants extends CvaVariants> = {
  readonly [Key in keyof Variants]?: Option<keyof Variants[Key]> | null | undefined;
};

/** External classes appended after selected options and compound variants. */
export type CvaClassProps = {
  readonly class?: StillwindClassValue;
  readonly className?: StillwindClassValue;
};

/** A conjunction of option conditions and the styles added when all conditions match. */
export type CvaCompound<Variants extends CvaVariants> = {
  readonly [Key in keyof Variants]?:
    | Option<keyof Variants[Key]>
    | null
    | undefined
    | readonly (Option<keyof Variants[Key]> | null | undefined)[];
} & {
  readonly class?: CvaInput;
  readonly className?: CvaInput;
};

/** Static configuration for a CVA factory. The class prop names cannot be variant axes. */
export type CvaConfig<Variants extends CvaVariants> = {
  readonly variants?: Variants & { readonly class?: never; readonly className?: never };
  readonly defaultVariants?: CvaSelection<Variants>;
  readonly compoundVariants?: readonly CvaCompound<Variants>[];
};

/** Framework props for an element; CVA results cannot be composed through style(). */
export type CvaResult<Props> = Pick<Props, Extract<keyof Props, "class" | "className" | "style">>;

/** Extracts variant choices from a CVA factory, excluding external class inputs. */
export type VariantProps<Factory extends (...args: never[]) => unknown> = Omit<
  Exclude<Parameters<Factory>[0], undefined>,
  "class" | "className"
>;
