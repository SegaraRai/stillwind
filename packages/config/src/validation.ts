/** Schema validation that reports authored field paths and keeps supplied objects unchanged. */

import * as v from "valibot";

export const nonemptyText = v.pipe(
  v.string(),
  v.check((value) => Boolean(value.trim()), "must be a nonempty string"),
);

export const nonemptyStrings = v.array(nonemptyText);

export const nonemptyStringList = v.pipe(
  nonemptyStrings,
  v.minLength(1, "must be an array of nonempty strings"),
);

/** Checks a schema without replacing objects or callbacks with the parser's output. */
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

/** Requires a record and rejects keys outside the declared entries. */
export function strictObject<T extends v.ObjectEntries>(entries: T) {
  return v.pipe(
    v.custom<Record<string, unknown>>(object, "must be an object"),
    v.strictObject(entries),
  );
}

export function object(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
