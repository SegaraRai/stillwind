/** Schema validation that reports authored field paths and keeps supplied objects unchanged. */

import * as v from "valibot";

/** Accepts strings with non-whitespace content while preserving all authored whitespace. */
export const nonemptyText = v.pipe(
  v.string(),
  v.check((value) => Boolean(value.trim()), "must be a nonempty string"),
);

/** Accepts arrays of strings with non-whitespace content. Arrays can be empty, and entries are not trimmed. */
export const nonemptyStrings = v.array(nonemptyText);

/** Requires at least one string with non-whitespace content, without trimming entries. */
export const nonemptyStringList = v.pipe(
  nonemptyStrings,
  v.minLength(1, "must be an array of nonempty strings"),
);

/**
 * Checks a schema and narrows the supplied value without replacing it with parsed output.
 * @param schema - Synchronous schema whose output type describes the input's existing shape.
 * Parsed replacements are discarded, so narrowing must not depend on a transformation.
 * @param value - Value to validate. Objects, arrays, and callbacks retain their original identities.
 * @param location - Root field path prepended to the first validation issue's property and array path.
 * @throws If validation fails. The error includes the first issue's field path and message.
 */
export function validate<T extends v.GenericSchema>(
  schema: T,
  value: unknown,
  location: string,
): asserts value is v.InferOutput<T> {
  const result = v.safeParse(schema, value, { abortEarly: true });
  if (result.success) {
    return;
  }

  const issue = result.issues[0];
  const suffix =
    issue.path
      ?.map(({ key }) => (typeof key === "number" ? `[${key}]` : `.${String(key)}`))
      .join("") ?? "";
  throw new Error(`${location}${suffix}: ${issue.message}`);
}

/**
 * Constructs a schema that requires an object and rejects undeclared keys.
 * @param entries - Field names and synchronous schemas used to validate their values.
 * @returns A schema that validates declared fields on non-null, non-array objects and rejects unknown keys.
 */
export function strictObject<T extends v.ObjectEntries>(entries: T) {
  return v.pipe(
    v.custom<Record<string, unknown>>(object, "must be an object"),
    v.strictObject(entries),
  );
}

/**
 * Checks whether a value can supply configuration fields.
 * @param value - Any runtime value. Objects with custom prototypes are accepted.
 * @returns Whether the value is a non-null object other than an array.
 */
export function object(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
