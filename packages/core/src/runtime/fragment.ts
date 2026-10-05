/**
 * CSS property names mapped to value text for a generated fragment's style or custom properties.
 */
export type StillwindStylePayload = Readonly<Record<string, string>>;

/** Framework conventions used when materializing generated fragments as element props. */
export interface FragmentRuntimeOptions {
  /** Element prop that receives class text, such as `class` or `className`. */
  readonly classProperty: string;
  /** Whether style records remain objects or become semicolon-separated declaration text. */
  readonly styleFormat: "object" | "string";
}

/**
 * Operations and private symbol keys shared by fragments created for one runtime instance.
 * Fragments must be merged by the instance that supplied their keys.
 */
export interface FragmentRuntime {
  /**
   * Own symbol property containing a fragment's class text. Other runtime instances use a different
   * key.
   */
  readonly stillwindClassKey: symbol;
  /** Own symbol property containing the fragment's authored style payload, before serialization. */
  readonly stillwindStyleKey: symbol;
  /**
   * Own symbol property containing generated custom properties applied after the authored style.
   */
  readonly stillwindCustomPropertiesKey: symbol;
  /**
   * Converts an authored style payload using this runtime's framework convention.
   *
   * @param value - Style payload. Null and undefined mean no style. String mode accepts text or an
   * object.
   * @returns The payload unchanged in object mode, serialized declarations in string mode, or
   * undefined for an unsupported string-mode payload.
   */
  readonly stillwindStyleValue: (value: unknown) => unknown;
  /**
   * Materializes fragments and ordinary prop records into one element prop object.
   *
   * Nested arrays are traversed in order. Fragment classes are joined. Later ordinary props and
   * authored style payloads replace earlier ones. Generated custom properties are combined and
   * applied last. The last non-null fragment style overrides ordinary style props. Inputs are not
   * mutated.
   *
   * @param values - Fragments from this instance, ordinary prop records, or nested arrays.
   * Nonobjects are ignored.
   * @returns Element props with this runtime's class property and style format.
   * @throws In development if an unrecognized record contains symbols or an own class prop, or the
   * result retains symbol properties.
   */
  readonly stillwindMerge: (...values: readonly unknown[]) => Record<string, unknown>;
  /**
   * Converts materialized element props into a fragment that this instance can merge.
   *
   * @param value - Element prop object. Class and style props become private symbol properties.
   * Other enumerable string props are copied.
   * @returns The input unchanged if it already has one of this instance's own fragment keys,
   * otherwise a new fragment.
   * @throws If the value is null or is not an object.
   */
  readonly stillwindToFragment: (value: unknown) => Record<PropertyKey, unknown>;
}

/**
 * Selects runtime validation when `NODE_ENV` is not exactly `production`.
 *
 * @returns True when the process or its environment is unavailable, or the environment is not
 * production.
 */
function isDevelopment(): boolean {
  return (
    typeof process === "undefined" ||
    process.env === undefined ||
    process.env.NODE_ENV !== "production"
  );
}

/**
 * Checks whether a value can carry fragment or style properties.
 *
 * @param value - Value to inspect.
 * @returns True for non-null objects, including arrays, and false for functions and primitives.
 */
function isObject(value: unknown): value is Record<PropertyKey, unknown> {
  return typeof value === "object" && value !== null;
}

/**
 * Creates framework-specific fragment operations with fresh symbol keys.
 *
 * @param options - Class prop name and style serialization convention for the target framework.
 * @returns An independent runtime. Generated fragments must use these keys when passed to its merge
 * operation.
 */
export function createFragmentRuntime(options: FragmentRuntimeOptions): FragmentRuntime {
  const stillwindClassKey = Symbol("stillwind.class");
  const stillwindStyleKey = Symbol("stillwind.style");
  const stillwindCustomPropertiesKey = Symbol("stillwind.custom-properties");
  const { classProperty, styleFormat } = options;

  const serialize = (payload: unknown): unknown => {
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
  };

  const merge = (...values: readonly unknown[]): Record<string, unknown> => {
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
      const symbols = Object.getOwnPropertySymbols(value);
      const fragment =
        symbols.includes(stillwindClassKey) ||
        symbols.includes(stillwindStyleKey) ||
        symbols.includes(stillwindCustomPropertiesKey);
      if (isDevelopment() && !fragment) {
        if (symbols.length > 0) {
          throw new TypeError("Stillwind received a fragment from a different runtime instance.");
        }
        if (Object.hasOwn(value, classProperty)) {
          throw new TypeError(
            `Stillwind received a materialized props object; convert it with stillwindToFragment() first.`,
          );
        }
      }
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
      const materialized = serialize(style);
      if (materialized !== undefined) {
        result.style = materialized;
      }
    }
    if (Object.keys(customProperties).length > 0) {
      result.style =
        styleFormat === "string"
          ? [serialize(result.style), serialize(customProperties)].filter(Boolean).join(";")
          : { ...(isObject(result.style) ? result.style : {}), ...customProperties };
    }
    if (isDevelopment() && Object.getOwnPropertySymbols(result).length > 0) {
      throw new TypeError("Stillwind produced a fragment that was not materialized.");
    }
    return result;
  };

  const toFragment = (value: unknown): Record<PropertyKey, unknown> => {
    if (!isObject(value)) {
      throw new TypeError("Stillwind expected a materialized props object.");
    }
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
  };

  return {
    stillwindClassKey,
    stillwindStyleKey,
    stillwindCustomPropertiesKey,
    stillwindStyleValue: (value) => (value == null ? undefined : serialize(value)),
    stillwindMerge: merge,
    stillwindToFragment: toFragment,
  };
}
