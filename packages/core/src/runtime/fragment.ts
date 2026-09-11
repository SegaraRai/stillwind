/**
 * Per-framework fragment runtime. Compiled `style()` calls build a fragment
 * (class contributions under a symbol key, the style payload under another)
 * and materialize it exactly once, at the call site.
 */

export type StillwindStylePayload = Readonly<Record<string, string>>;

export interface FragmentRuntimeOptions {
  /** `className` for React, `class` for every other framework. */
  readonly classProperty: string;
  /** Object frameworks receive the payload as an object, others as a string. */
  readonly styleFormat: "object" | "string";
}

export interface FragmentRuntime {
  readonly stillwindClassKey: symbol;
  readonly stillwindStyleKey: symbol;
  readonly stillwindMerge: (...values: readonly unknown[]) => Record<string, unknown>;
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

export function createFragmentRuntime(options: FragmentRuntimeOptions): FragmentRuntime {
  const stillwindClassKey = Symbol("stillwind.class");
  const stillwindStyleKey = Symbol("stillwind.style");
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
      const fragment = symbols.includes(stillwindClassKey) || symbols.includes(stillwindStyleKey);
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
    if (isDevelopment() && Object.getOwnPropertySymbols(result).length > 0) {
      throw new TypeError("Stillwind produced a fragment that was not materialized.");
    }
    return result;
  };

  const toFragment = (value: unknown): Record<PropertyKey, unknown> => {
    if (!isObject(value)) {
      throw new TypeError("Stillwind expected a materialized props object.");
    }
    if (Object.hasOwn(value, stillwindClassKey) || Object.hasOwn(value, stillwindStyleKey)) {
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
    stillwindMerge: merge,
    stillwindToFragment: toFragment,
  };
}
