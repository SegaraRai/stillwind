/**
 * A static variant prefix used as an object key in `style()`, including its final `:`.
 * Nested keys compose from outermost to innermost. Raw selector keys must use
 * arbitrary-variant syntax, such as `[&:hover]:`.
 */
export type StyleVariant = `${string}:`;

/**
 * Ordinary values for the external-class entry `{ class: value }` in `style()`.
 *
 * Strings, numbers, and bigints become class text. Arrays are flattened, and object
 * keys are included when their values are truthy. Booleans, null, and undefined add no
 * class text. These values may be stored and passed to other functions.
 */
export type StillwindClassValue =
  | string
  | number
  | bigint
  | boolean
  | null
  | undefined
  | readonly StillwindClassValue[]
  | Readonly<Record<string, unknown>>;

/**
 * Ordinary values for inline custom properties.
 *
 * These may be stored and passed between functions. Strings are CSS text, numbers have
 * no unit, and URL objects become escaped `url("...")` values. True becomes one space,
 * and false becomes `initial`.
 */
export type StyleCustomPropertyValue = URL | string | number | boolean;
