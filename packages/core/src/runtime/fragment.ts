/**
 * Fragment construction and props materialization for compiled style calls.
 * Each runtime instance owns its class and style symbols; fragments must be
 * consumed by the instance that created their keys.
 */

export type StillwindStylePayload = Readonly<Record<string, string>>;

export interface FragmentRuntimeOptions {
  /** `className` for React, `class` for every other framework. */
  readonly classProperty: string;
  /** Object frameworks receive the payload as an object, others as a string. */
  readonly styleFormat: "object" | "string";
}

/**
 * Instance-local fragment keys and conversion operations. Compiler-generated
 * fragments carry class contributions and an optional complete style payload.
 */
export interface FragmentRuntime {
  readonly stillwindClassKey: symbol;
  readonly stillwindStyleKey: symbol;
  /** Custom properties merge individually, with later entries winning. */
  readonly stillwindCustomPropertiesKey: symbol;
  /**
   * Flattens arrays left-to-right, ignoring falsy and nonobject values. Class
   * contributions are joined without deduplication; later enumerable props win.
   * The last non-null symbol style payload replaces the entire earlier payload
   * and, when serializable, takes precedence over the ordinary style prop.
   * Custom-property fragments merge per name and overlay the materialized style.
   * Development checks reject foreign symbols and materialized class props on
   * objects without this instance's keys; production omits those checks.
   */
  readonly stillwindMerge: (...values: readonly unknown[]) => Record<string, unknown>;
  /**
   * Returns an existing instance fragment unchanged, otherwise copies enumerable
   * string props and moves class/style to the instance symbols. Other symbol keys
   * are not copied. Throws TypeError for nonobjects in every build mode.
   */
  readonly stillwindToFragment: (value: unknown) => Record<PropertyKey, unknown>;
}

function isDevelopment(): boolean {
  return (
    typeof process === "undefined" ||
    process.env === undefined ||
    process.env.NODE_ENV !== "production"
  );
}

function isObject(value: unknown): value is Record<PropertyKey, unknown> {
  return typeof value === "object" && value !== null;
}

/**
 * Creates fresh fragment symbols and operations for the supplied props convention.
 * String style serialization joins key:value entries with semicolons without CSS
 * escaping; callers must supply valid CSS names and values. Development checks are
 * enabled unless process.env.NODE_ENV is explicitly "production".
 */
export function createFragmentRuntime(options: FragmentRuntimeOptions): FragmentRuntime {
  const stillwindClassKey = Symbol("stillwind.class");
  const stillwindStyleKey = Symbol("stillwind.style");
  const stillwindCustomPropertiesKey = Symbol("stillwind.custom-properties");
  const { classProperty, styleFormat } = options;

  const serialize = (payload: unknown): unknown => {
    if (styleFormat === "string") {
      if (typeof payload === "string") return payload;
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
      if (!value) return;
      if (Array.isArray(value)) {
        for (const element of value) collect(element);
        return;
      }
      if (!isObject(value)) return;
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
        if (isObject(custom)) Object.assign(customProperties, custom);
        const contribution = value[stillwindClassKey];
        if (typeof contribution === "string" && contribution.length > 0) classes.push(contribution);
        if (Object.hasOwn(value, stillwindStyleKey)) {
          const payload = value[stillwindStyleKey];
          if (payload !== undefined && payload !== null) {
            style = payload;
            hasStyle = true;
          }
        }
      }
      for (const [key, entry] of Object.entries(value)) props[key] = entry;
    };

    for (const value of values) collect(value);

    const result: Record<string, unknown> = {};
    if (classes.length > 0) result[classProperty] = classes.join(" ");
    Object.assign(result, props);
    if (hasStyle) {
      const materialized = serialize(style);
      if (materialized !== undefined) result.style = materialized;
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
    stillwindMerge: merge,
    stillwindToFragment: toFragment,
  };
}
