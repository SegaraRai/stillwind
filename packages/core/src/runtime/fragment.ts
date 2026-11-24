import { stillwindClsx } from "./shared.ts";

/**
 * Framework conventions and private channels used to materialize fragments.
 * The four keys must be distinct and belong to the runtime that created the fragments.
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
  /** Own fragment property containing generated custom properties. */
  stillwindCustomPropertiesKey: symbol,
  /** Own fragment property containing props that a caller forwarded, applied after all others. */
  stillwindForwardKey: symbol,
];

/** Class and style props that a component received from its caller. */
interface ForwardedProps {
  /** Class text joined from the forwarded `class` and `className` values. */
  readonly class: string;
  /** The forwarded style value, unchanged. */
  readonly style: unknown;
  /** Whether the framework accepts an array of style values. */
  readonly styleArrays: boolean;
}

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
 * Merges a forwarded style value after the style that a call produced.
 *
 * Objects merge with the forwarded entries last, and strings are joined with `;`. When one
 * value is a string and the other is an object, a framework that accepts style arrays receives
 * both values in an array. Otherwise the object is serialized, because declaration text cannot
 * be parsed into an object reliably. A forwarded array is used only when `styleArrays` is true.
 *
 * @param own - The style that the call produced, or undefined.
 * @param forwarded - The style value that a caller forwarded.
 * @param styleFormat - The style convention of the output format.
 * @param styleArrays - Whether the framework accepts an array of style values.
 * @returns The merged style, or undefined when neither value contributes.
 */
export function mergeForwardedStyle(
  own: unknown,
  forwarded: unknown,
  styleFormat: "object" | "string",
  styleArrays: boolean,
): unknown {
  if (forwarded == null || forwarded === "" || (Array.isArray(forwarded) && !styleArrays)) {
    return own;
  }
  if (own == null || own === "") {
    return Array.isArray(forwarded) ? forwarded : serializeStyle(forwarded, styleFormat);
  }
  if (Array.isArray(own) || Array.isArray(forwarded)) {
    return [own, forwarded];
  }
  if (isObject(own) && isObject(forwarded)) {
    return {
      ...own,
      ...forwarded,
    };
  }
  if (styleArrays && isObject(own) !== isObject(forwarded)) {
    return [own, forwarded];
  }
  return [serializeStyle(own, "string"), serializeStyle(forwarded, "string")]
    .filter(Boolean)
    .join(";");
}

/**
 * Creates a fragment from the class and style props that a component received.
 *
 * @param options - The receiving framework's prop conventions and fragment keys.
 * @param value - Received props. Both `class` and `className` are read, as `stillwindClsx` reads
 * class values. Other props are ignored, and a value that is not an object contributes nothing.
 * @param styleArrays - Whether the framework accepts an array of style values.
 * @returns A fragment that `mergeFragments` applies after every other fragment.
 */
export function forwardFragment(
  options: FragmentRuntimeOptions,
  value: unknown,
  styleArrays: boolean,
): Record<PropertyKey, unknown> {
  if (!isObject(value)) {
    return {};
  }
  const forwarded: ForwardedProps = {
    class: stillwindClsx(value.class, value.className),
    style: value.style,
    styleArrays,
  };
  return {
    [options[5]]: forwarded,
  };
}

/**
 * Materializes class and style fragments without mutating the inputs.
 *
 * @param options - Framework conventions and the keys of the runtime that created the fragments.
 * @param values - Fragments or nested arrays.
 * Nonobjects are ignored. Materialized bases must first be converted with `toFragment`.
 * @returns Props with classes joined in traversal order. The last non-null fragment
 * style supplies inline style, and generated custom properties are combined and applied
 * after it. Forwarded fragments apply last, in traversal order: their classes follow the
 * other classes, and their styles merge as `mergeForwardedStyle` describes. Fragment
 * symbol channels do not appear in the result.
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
    stillwindForwardKey,
  ] = options;
  const classes: string[] = [];
  const forwards: ForwardedProps[] = [];
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
    if (Object.hasOwn(value, stillwindForwardKey)) {
      forwards.push(value[stillwindForwardKey] as ForwardedProps);
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
  };

  for (const value of values) {
    collect(value);
  }

  const result: Record<string, unknown> = {};
  if (classes.length > 0) {
    result[classProperty] = classes.join(" ");
  }
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
        : {
            ...(isObject(result.style) ? result.style : {}),
            ...customProperties,
          };
  }
  for (const forwarded of forwards) {
    if (forwarded.class) {
      result[classProperty] = result[classProperty]
        ? `${result[classProperty] as string} ${forwarded.class}`
        : forwarded.class;
    }
    const style = mergeForwardedStyle(
      result.style,
      forwarded.style,
      styleFormat,
      forwarded.styleArrays,
    );
    if (style !== undefined) {
      result.style = style;
    }
  }
  return result;
}

/**
 * Converts a composition base to a fragment that contributes only its class.
 *
 * A base contributes either `class` or `className`, or the class channel of a fragment
 * from this runtime. The receiving output's prop takes precedence unless it is null or
 * undefined. Style payloads and custom properties are ignored.
 *
 * @param options - The receiving framework's prop conventions and fragment keys.
 * @param value - A non-null object containing materialized props or this runtime's
 * fragment channels.
 * @returns A new fragment whose class channel holds the base's class value.
 */
export function toFragment(
  options: FragmentRuntimeOptions,
  value: Record<PropertyKey, unknown>,
): Record<PropertyKey, unknown> {
  const [classProperty, , stillwindClassKey] = options;
  return {
    [stillwindClassKey]: Object.hasOwn(value, stillwindClassKey)
      ? value[stillwindClassKey]
      : (value[classProperty] ?? value[classProperty === "class" ? "className" : "class"]),
  };
}
