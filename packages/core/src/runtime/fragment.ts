/**
 * Framework conventions and private channels used to materialize fragments.
 * The three keys must be distinct and belong to the runtime that created the fragments.
 */
export type FragmentRuntimeOptions = readonly [
  /** Element prop that receives the combined class text. */
  classProperty: "class" | "className",
  /** Whether style records become objects or semicolon-separated declaration text. */
  styleFormat: "object" | "string",
  /** Own fragment property containing class text. */
  stillwindClassKey: symbol,
  /** Own fragment property containing the last authored style payload. */
  stillwindStyleKey: symbol,
  /** Own fragment property containing generated custom properties applied last. */
  stillwindCustomPropertiesKey: symbol,
];

export function isObject(value: unknown): value is Record<PropertyKey, unknown> {
  return typeof value === "object" && value !== null;
}

/**
 * Converts a style payload using the framework's style convention.
 *
 * @param payload - Arbitrary payload. String mode accepts text and objects whose entries become declarations.
 * @param styleFormat - Object mode returns the payload unchanged; string mode serializes object entries in order.
 * @returns The payload, declaration text, or undefined for an unsupported string-mode payload.
 */
export function serializeStyle(payload: unknown, styleFormat: "object" | "string"): unknown {
  if (styleFormat === "string") {
    if (typeof payload === "string") {
      return payload;
    }
    if (isObject(payload)) {
      return Object.entries(payload)
        .map(([name, value]) => `${name}:${String(value)}`)
        .join(";");
    }
    return undefined;
  }
  return payload;
}

/**
 * Materializes fragments and ordinary props without mutating the inputs.
 *
 * @param options - Framework conventions and the keys of the runtime that created the fragments.
 * @param values - Fragments, ordinary prop records without an own class prop, or nested arrays.
 * Nonobjects are ignored. Materialized bases must first be converted with `toFragment`.
 * @returns Props with classes joined in traversal order. Later ordinary props and authored style payloads
 * replace earlier values. The last non-null fragment style overrides ordinary style props, and generated
 * custom properties are combined and applied last. Fragment symbol channels do not appear in the result.
 */
export function mergeFragments(
  options: FragmentRuntimeOptions,
  values: readonly unknown[],
): Record<string, unknown> {
  const [
    classProperty,
    styleFormat,
    stillwindClassKey,
    stillwindStyleKey,
    stillwindCustomPropertiesKey,
  ] = options;
  const classes: string[] = [];
  const props: Record<string, unknown> = {};
  const customProperties: Record<string, string> = {};
  let style: unknown;
  let hasStyle = false;

  const collect = (value: unknown): void => {
    if (!value) {
      return;
    }
    if (Array.isArray(value)) {
      for (const element of value) {
        collect(element);
      }
      return;
    }
    if (!isObject(value)) {
      return;
    }
    const fragment =
      Object.hasOwn(value, stillwindClassKey) ||
      Object.hasOwn(value, stillwindStyleKey) ||
      Object.hasOwn(value, stillwindCustomPropertiesKey);
    if (fragment) {
      const custom = value[stillwindCustomPropertiesKey];
      if (isObject(custom)) {
        Object.assign(customProperties, custom);
      }
      const contribution = value[stillwindClassKey];
      if (typeof contribution === "string" && contribution.length > 0) {
        classes.push(contribution);
      }
      if (Object.hasOwn(value, stillwindStyleKey)) {
        const payload = value[stillwindStyleKey];
        if (payload !== undefined && payload !== null) {
          style = payload;
          hasStyle = true;
        }
      }
    }
    for (const [key, entry] of Object.entries(value)) {
      props[key] = entry;
    }
  };

  for (const value of values) {
    collect(value);
  }

  const result: Record<string, unknown> = {};
  if (classes.length > 0) {
    result[classProperty] = classes.join(" ");
  }
  Object.assign(result, props);
  if (hasStyle) {
    const materialized = serializeStyle(style, styleFormat);
    if (materialized !== undefined) {
      result.style = materialized;
    }
  }
  if (Object.keys(customProperties).length > 0) {
    result.style =
      styleFormat === "string"
        ? [serializeStyle(result.style, styleFormat), serializeStyle(customProperties, styleFormat)]
            .filter(Boolean)
            .join(";")
        : { ...(isObject(result.style) ? result.style : {}), ...customProperties };
  }
  return result;
}

/**
 * Converts materialized props to private fragment channels without mutating them.
 *
 * @param options - The receiving framework's prop conventions and fragment keys.
 * @param value - A non-null object containing materialized props or this runtime's fragment channels.
 * @returns The input when it already owns a fragment key; otherwise a new object whose class and style
 * props use fragment keys and whose other enumerable string props are copied.
 */
export function toFragment(
  options: FragmentRuntimeOptions,
  value: Record<PropertyKey, unknown>,
): Record<PropertyKey, unknown> {
  const [classProperty, , stillwindClassKey, stillwindStyleKey, stillwindCustomPropertiesKey] =
    options;
  if (
    Object.hasOwn(value, stillwindClassKey) ||
    Object.hasOwn(value, stillwindStyleKey) ||
    Object.hasOwn(value, stillwindCustomPropertiesKey)
  ) {
    return value;
  }
  const fragment: Record<PropertyKey, unknown> = {};
  for (const [key, entry] of Object.entries(value)) {
    if (key === classProperty) {
      fragment[stillwindClassKey] = entry;
      continue;
    }
    if (key === "style") {
      fragment[stillwindStyleKey] = entry;
      continue;
    }
    fragment[key] = entry;
  }
  return fragment;
}
