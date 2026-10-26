import type { NativeLegacyThemeKey, NativeThemeToken } from "@stillwind/native";
import defaultTheme from "tailwindcss/defaultTheme";

import { serializeCss, splitTopLevel, withAlpha } from "./css.ts";

type Dictionary = Record<string, unknown>;

/**
 * The `theme()` function that Tailwind passes to plugins and to functions in theme config.
 */
type ThemeFunction = ((path?: string, fallback?: unknown) => unknown) & {
  /** The function itself, for config functions written as `({ theme }) => ...`. */
  theme: ThemeFunction;
  /** The resolved `colors` section, for config functions written as `({ colors }) => ...`. */
  colors: unknown;
};

// The tables, `keyPathToCssProperty()`, `themeableValues()`, `readFromCss()`, and the merge
// rules of `get` follow Tailwind CSS 4.3.3 (compat/apply-config-to-theme.ts and
// compat/plugin-functions.ts). Keep them aligned with upstream instead of adding names here:
// they define which v3 config key reads and writes which CSS theme variable. The v3 config
// keys whose namespace has another name come from the `@stillwind-legacy-theme-key`
// statements of the design system.

/**
 * Config keys whose `DEFAULT` value is a shared default in the `--default-*` namespace.
 */
const SPECIAL_DEFAULT_KEYS: Record<string, string> = {
  borderWidth: "border-width",
  outlineWidth: "outline-width",
  ringColor: "ring-color",
  ringWidth: "ring-width",
  transitionDuration: "transition-duration",
  transitionTimingFunction: "transition-timing-function",
};

const IS_VALID_KEY = /^[a-zA-Z0-9-_%/.]+$/;

/** Tailwind's `ThemeOptions.DEFAULT` flag in `__CSS_VALUES__`. */
const DEFAULT_OPTION = 4;

/**
 * Converts a config key path to the name of its CSS theme variable without `--`, as Tailwind does.
 * `fontSize.xs.1.lineHeight` becomes `text-xs--line-height`, and `transitionDuration.DEFAULT`
 * becomes `default-transition-duration`.
 *
 * @param path - Config keys, such as `["colors", "red", "500"]`.
 * @param namespaces - The namespace name of each v3 config key whose namespace has another
 * name, such as `text` for `fontSize`. Other keys use their kebab-case name.
 * @returns The variable name, or null for the legacy container config and for keys with
 * characters that a variable name cannot hold.
 */
export function keyPathToCssProperty(
  path: readonly string[],
  namespaces: Readonly<Record<string, string>>,
): string | null {
  const special = SPECIAL_DEFAULT_KEYS[path[0]];
  if (special && path[1] === "DEFAULT") {
    return `default-${special}`;
  }
  if (path[0] === "container" || !path.every((part) => IS_VALID_KEY.test(part))) {
    return null;
  }
  const parts = [namespaces[path[0]] ?? path[0], ...path.slice(1)];
  return parts
    .map((part, index, all) => (part === "1" && index !== all.length - 1 ? "" : part))
    .map((part, index) => {
      part = part.replaceAll(".", "_");
      return index === 0 || part.startsWith("-") || part === "lineHeight"
        ? part.replace(/([a-z])([A-Z])/g, (_, a: string, b: string) => `${a}-${b.toLowerCase()}`)
        : part;
    })
    .filter((part, index) => part !== "DEFAULT" || index !== path.length - 1)
    .join("-");
}

/**
 * Lists the primitive values of a resolved theme config with their key paths, as Tailwind writes
 * them back to CSS: tuples add their extra values under `-key`, and string arrays become a
 * comma-separated list, except that a `fontSize` array is a size and a line height.
 *
 * @param theme - Resolved config sections keyed by config key.
 * @returns Key paths and values in traversal order. Keys that start with `__` are skipped.
 */
function themeableValues(theme: Dictionary): [string[], unknown][] {
  const result: [string[], unknown][] = [];
  /**
   * Adds the primitive values of one config value.
   * @param value - Config value at `path`.
   * @param path - Keys from the section key to `value`.
   */
  const visit = (value: unknown, path: string[]) => {
    if (typeof value === "string" || typeof value === "number") {
      result.push([path, value]);
    } else if (
      Array.isArray(value) &&
      value.length === 2 &&
      (typeof value[0] === "string" || typeof value[0] === "number") &&
      object(value[1])
    ) {
      result.push([path, value[0]]);
      for (const [key, extra] of Object.entries(value[1])) {
        result.push([[...path, `-${key}`], extra]);
      }
    } else if (Array.isArray(value) && value.every((entry) => typeof entry === "string")) {
      if (path[0] === "fontSize") {
        result.push([path, value[0]]);
        if (value.length >= 2) {
          result.push([[...path, "-line-height"], value[1]]);
        }
      } else {
        result.push([path, value.join(", ")]);
      }
    } else if (object(value)) {
      for (const [key, entry] of Object.entries(value)) {
        if (!key.startsWith("__")) {
          visit(entry, [...path, key]);
        }
      }
    }
  };
  visit(theme, []);
  return result;
}

/**
 * Parses a `theme()` path, as Tailwind's `toKeyPath()` does.
 *
 * @param path - Dotted path with optional brackets, such as `colors.red[500]`.
 * @returns The keys, such as `["colors", "red", "500"]`. A bracket keeps dots, so `spacing[1.5]`
 * has the key `1.5`.
 */
export function toKeyPath(path: string): string[] {
  const keys: string[] = [];
  for (const part of splitTopLevel(path, ".")) {
    let index = 0;
    for (;;) {
      const open = part.indexOf("[", index);
      const close = part.indexOf("]", open);
      if (open < 0 || close < 0) {
        break;
      }
      if (open > index) {
        keys.push(part.slice(index, open));
      }
      keys.push(part.slice(open + 1, close));
      index = close + 1;
    }
    if (index < part.length) {
      keys.push(part.slice(index));
    }
  }
  return keys;
}

/**
 * Reads a key path from a config object. A missing key joins the next one with `-`, so
 * `colors.red.500` also finds a `red-500` entry.
 *
 * @param value - Config object.
 * @param path - Keys below `value`.
 * @returns The value at the path, or undefined when a key is missing at the end of the path.
 */
function read(value: unknown, path: readonly string[]): unknown {
  const keys = [...path];
  for (let index = 0; index < keys.length; index++) {
    const key = keys[index];
    if (!object(value) || !Object.hasOwn(value, key)) {
      if (keys[index + 1] === undefined) {
        return undefined;
      }
      keys[index + 1] = `${key}-${keys[index + 1]}`;
      continue;
    }
    value = value[key];
  }
  return value;
}

/**
 * Whether a theme value is a `var()` reference to the variable that it would define.
 *
 * @param name - Variable name without the leading `--`, such as `color-primary`.
 * @param value - Configured value.
 * @returns Whether `value` contains `var(--name)` or `var(--name, ...)`, which makes a declaration
 * of `--name` a cycle.
 */
function selfReference(name: string, value: unknown): boolean {
  return (
    typeof value === "string" &&
    [...value.matchAll(/var\(\s*--([^\s,)]+)\s*[,)]/g)].some((reference) => reference[1] === name)
  );
}

/**
 * Whether a value is a plain object rather than null or an array.
 *
 * @param value - Any value.
 * @returns Whether `value` can hold config keys.
 */
function object(value: unknown): value is Dictionary {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/**
 * Merges configuration objects deeply. Arrays and scalar values replace earlier values.
 *
 * @param target - Object that receives the entries; nested objects are copied before merging.
 * @param source - Entries to merge into `target`.
 * @returns `target`.
 */
export function merge(target: Dictionary, source: Dictionary): Dictionary {
  for (const [key, value] of Object.entries(source)) {
    target[key] =
      object(value) && object(target[key])
        ? merge(
            {
              ...target[key],
            },
            value,
          )
        : value;
  }
  return target;
}

/**
 * Reads one `fontFamily` config entry, as Tailwind's `fontFamily.sans` handling does.
 *
 * @param value - Config entry: a family string, an array of families, or a `[family, settings]`
 * tuple.
 * @param setting - Settings key of a tuple, such as `fontFeatureSettings`, or undefined for the
 * family.
 * @returns The family with array entries joined by `, `, or the tuple's setting. Returns null when
 * the entry does not hold the requested value.
 */
function fontValue(value: unknown, setting?: string): string | null {
  if (Array.isArray(value) && value.length === 2 && object(value[1])) {
    const result = setting === undefined ? value[0] : value[1][setting];
    return result === undefined || result === null ? null : String(result);
  }
  if (setting !== undefined) {
    return null;
  }
  if (Array.isArray(value)) {
    return value.join(", ");
  }
  return typeof value === "string" ? value : null;
}

/**
 * The `theme` and `theme.extend` objects of one config.
 */
interface ConfigLayer {
  theme: Dictionary;
  extend: Dictionary;
}

/**
 * Resolves plugin theme configuration against the active CSS tokens, as Tailwind's compatibility
 * layer does, and writes the plugins' own configuration back as CSS theme variables.
 *
 * Reads combine three sources: CSS tokens, the plugins' config, and Tailwind's v3 default theme
 * without colors. Ordinary CSS definitions win. Config values win over `@theme default`
 * definitions and over the defaults.
 */
export class PluginTheme {
  /** The function passed to plugin handlers and config functions. */
  readonly get: ThemeFunction;

  /** Active tokens by full name. */
  readonly #tokens: ReadonlyMap<string, NativeThemeToken>;

  /** The namespace name of each v3 config key whose namespace has another name. */
  readonly #namespaces: Readonly<Record<string, string>>;

  /** Tailwind's compatibility defaults followed by the plugin configs, in plugin order. */
  readonly #layers: ConfigLayer[];

  /** Number of leading layers that are compatibility defaults rather than plugin config. */
  readonly #defaults: number;

  readonly #sections = new Map<string, unknown>();

  readonly #resolving = new Set<string>();

  /**
   * @param configs - Configs of the plugins and their presets, in registration order.
   * @param tokens - Active CSS theme tokens.
   * @param legacyKeys - The namespaces that v3 config keys read, from the design system.
   */
  constructor(
    configs: readonly Dictionary[],
    tokens: readonly NativeThemeToken[],
    legacyKeys: readonly NativeLegacyThemeKey[],
  ) {
    this.#tokens = new Map(tokens.map((token) => [token.name, token]));
    this.#namespaces = Object.fromEntries(legacyKeys.map(({ key, namespace }) => [key, namespace]));
    /**
     * Splits a config's `theme` into its replacing and extending parts.
     * @param theme - The `theme` value of a config.
     * @returns The layer, empty for a value that is not an object.
     */
    const layer = (theme: unknown): ConfigLayer => {
      const { extend, ...rest } = object(theme) ? theme : {};
      return {
        theme: rest,
        extend: object(extend) ? extend : {},
      };
    };
    /**
     * Creates a config function that reads one CSS theme namespace.
     * @param namespace - CSS namespace, such as `color`.
     * @returns A function that returns the namespace's values, or an empty object.
     */
    const fromCss = (namespace: string) => () => this.get(namespace, {});
    // Tailwind's `createCompatConfig()`: the v3 defaults, and the CSS namespaces exposed under
    // their v3 keys. A `replace` key, such as `colors`, reads only CSS. An `extend` key adds
    // the CSS values to the v3 defaults.
    const exposed = (exposure: NativeLegacyThemeKey["exposure"]) =>
      Object.fromEntries(
        legacyKeys
          .filter((legacy) => legacy.exposure === exposure)
          .map(({ key, namespace }) => [key, fromCss(namespace)]),
      );
    this.#layers = [
      {
        theme: {
          ...(defaultTheme as Dictionary),
          ...exposed("replace"),
        },
        extend: {
          ...exposed("extend"),
          transitionDuration: {
            DEFAULT: this.#tokens.get("--default-transition-duration")?.value ?? null,
          },
          transitionTimingFunction: {
            DEFAULT: this.#tokens.get("--default-transition-timing-function")?.value ?? null,
          },
        },
      },
      ...configs.map((config) => layer(config.theme)),
    ];
    this.#defaults = 1;
    /**
     * The theme function of plugins and config functions.
     * @param path - Config path, or undefined for every section.
     * @param fallback - Value for a path that resolves to nothing.
     * @returns The resolved value.
     */
    const get = ((path?: string, fallback?: unknown) =>
      path === undefined ? this.#all() : this.#theme(path, fallback)) as ThemeFunction;
    get.theme = get;
    this.get = get;
    get.colors = get("colors");
  }

  /**
   * Emits the plugins' own theme configuration, which native utility resolution also needs.
   *
   * @returns A `@stillwind-plugin-theme` block with the configured variables and keyframes, and a
   * `@stillwind-plugin-theme inline` block with the values that refer to their own variable, or an
   * empty string when the plugins configure nothing.
   */
  css(): string {
    const configured: Dictionary = {};
    const replaced = new Set<string>();
    const layers = this.#layers.slice(this.#defaults);
    for (const key of new Set(
      layers.flatMap(({ theme, extend }) => [...Object.keys(theme), ...Object.keys(extend)]),
    )) {
      if (layers.some(({ theme }) => Object.hasOwn(theme, key))) {
        replaced.add(key);
      }
      configured[key] = this.#resolve(key, layers);
    }
    let declarations = "";
    let inline = "";
    // A replaced section clears the `@theme default` values of its namespace, as Tailwind's
    // `clearNamespace(..., ThemeOptions.DEFAULT)` does. Ordinary definitions stay.
    for (const key of replaced) {
      const name = keyPathToCssProperty([key], this.#namespaces);
      if (name) {
        declarations += `--${name}-*:initial;`;
      }
    }
    for (const [path, configuredValue] of themeableValues(configured)) {
      let value = configuredValue;
      if (path[0] === "keyframes") {
        continue;
      }
      if (typeof value === "string") {
        value = value.replaceAll("<alpha-value>", "1");
      }
      if (path[0] === "opacity") {
        const number = typeof value === "string" ? Number.parseFloat(value) : Number(value);
        if (number >= 0 && number <= 1) {
          value = `${number * 100}%`;
        }
      }
      const name = keyPathToCssProperty(path, this.#namespaces);
      if (!name) {
        continue;
      }
      // A value such as daisyUI's `colors.primary: "var(--color-primary)"` names the variable
      // that it would define. Emitting it would make the variable invalid through a cycle, so
      // utilities inline the reference and the plugin's own CSS keeps defining the variable.
      if (selfReference(name, value)) {
        inline += `--${name}:${String(value)};`;
      } else {
        declarations += `--${name}:${String(value)};`;
      }
    }
    // Tailwind points the preflight fonts of documents and code at the configured `sans` and
    // `mono` families, so they apply even without the default theme. An ordinary `--font-sans` or
    // `--font-mono` definition keeps the preflight defaults unchanged.
    if (object(configured.fontFamily)) {
      for (const [key, prefix] of [
        ["sans", "default"],
        ["mono", "default-mono"],
      ]) {
        const entry = configured.fontFamily[key];
        const family = fontValue(entry);
        const token = this.#tokens.get(`--font-${key}`);
        if (family !== null && (!token || token.fallback)) {
          declarations +=
            `--${prefix}-font-family:${family};` +
            `--${prefix}-font-feature-settings:${fontValue(entry, "fontFeatureSettings") ?? "normal"};` +
            `--${prefix}-font-variation-settings:${fontValue(entry, "fontVariationSettings") ?? "normal"};`;
        }
      }
    }
    let frames = "";
    if (object(configured.keyframes)) {
      for (const [name, body] of Object.entries(configured.keyframes)) {
        frames += `@keyframes ${name}{${serializeCss(body)}}`;
      }
    }
    return [
      declarations || frames ? `@stillwind-plugin-theme{${declarations}${frames}}` : "",
      inline ? `@stillwind-plugin-theme inline{${inline}}` : "",
    ].join("");
  }

  /**
   * Resolves every section, for `config("theme")` and `theme()` without a path.
   *
   * @returns Resolved sections keyed by config key.
   */
  #all(): Dictionary {
    const keys = new Set(
      this.#layers.flatMap(({ theme, extend }) => [...Object.keys(theme), ...Object.keys(extend)]),
    );
    return Object.fromEntries([...keys].map((key) => [key, this.#section(key)]));
  }

  /**
   * Resolves one config section across all layers, once.
   *
   * @param key - Config key, such as `colors`.
   * @returns The resolved section, or undefined when no layer configures it.
   * @throws If resolving the section reads the section itself.
   */
  #section(key: string): unknown {
    if (this.#sections.has(key)) {
      return this.#sections.get(key);
    }
    if (this.#resolving.has(key)) {
      throw new Error(`Circular Tailwind plugin theme reference: ${key}`);
    }
    this.#resolving.add(key);
    try {
      const value = this.#resolve(key, this.#layers);
      this.#sections.set(key, value);
      return value;
    } finally {
      this.#resolving.delete(key);
    }
  }

  /**
   * Resolves one section of `layers`: the last `theme` value replaces earlier ones, and every
   * `extend` value then merges into it. Function values receive the theme function.
   *
   * @param key - Config key.
   * @param layers - Layers to combine, in registration order.
   * @returns The resolved section, or undefined when no layer configures it.
   */
  #resolve(key: string, layers: readonly ConfigLayer[]): unknown {
    /**
     * Calls a config function with the theme function.
     * @param value - Config value or function.
     * @returns The function's result, or `value` itself.
     */
    const evaluate = (value: unknown) =>
      typeof value === "function" ? (value as (theme: ThemeFunction) => unknown)(this.get) : value;
    let value: unknown;
    for (const layer of layers) {
      if (Object.hasOwn(layer.theme, key)) {
        value = evaluate(layer.theme[key]);
      }
    }
    for (const layer of layers) {
      if (Object.hasOwn(layer.extend, key)) {
        const extension = evaluate(layer.extend[key]);
        value = object(extension)
          ? merge(
              {
                ...(object(value) ? value : {}),
              },
              extension,
            )
          : extension;
      }
    }
    return value;
  }

  /**
   * Tailwind's `theme(path, defaultValue)` for plugins.
   *
   * @param path - Config path, or a CSS variable name such as `--color-red-500`, with an optional
   * trailing `/ alpha`.
   * @param fallback - Value for a path that resolves to nothing.
   * @returns The resolved value, with the alpha applied to a string value.
   */
  #theme(path: string, fallback: unknown): unknown {
    const slash = path.lastIndexOf("/");
    const modifier = slash < 0 ? null : path.slice(slash + 1).trim();
    const keys = toKeyPath(slash < 0 ? path : path.slice(0, slash).trim());
    const [css, options] = this.#readFromCss(keys);
    let config = keys[0] === undefined ? undefined : read(this.#section(keys[0]), keys.slice(1));
    if (typeof config === "string") {
      config = config.replace("<alpha-value>", "1");
    }
    let value: unknown;
    if (css === null) {
      value = config;
    } else if (typeof css !== "object") {
      // A `@theme default` value yields to the config, as it does in Tailwind.
      value = typeof options === "number" && options & DEFAULT_OPTION ? (config ?? css) : css;
    } else if (object(config) && object(css)) {
      const copy = merge({}, config);
      const cssOptions = (css.__CSS_VALUES__ ?? {}) as Record<string, number>;
      for (const [key, entry] of Object.entries(css)) {
        const yields =
          ((cssOptions[key] ?? 0) & DEFAULT_OPTION) !== 0 &&
          read(copy, key.split("-")) !== undefined;
        if (key !== "__CSS_VALUES__" && !yields) {
          copy[key] = entry;
        }
      }
      value = copy;
    } else {
      // Both `--color-foo` and `--color-foo-bar` make `colors.foo` an object; Tailwind returns
      // its `DEFAULT` to plugins.
      value = object(css) && "DEFAULT" in css ? css.DEFAULT : css;
    }
    if (modifier && typeof value === "string") {
      value = withAlpha(value, modifier);
    }
    return value ?? fallback;
  }

  /**
   * Tailwind's `readFromCss()`: the token of a key path, or the namespace's tokens as an object
   * keyed by suffix with `__CSS_VALUES__` options. `--text-xs--line-height` becomes the tuple
   * `[size, { lineHeight }]` of `xs`.
   *
   * @param path - Config key path, or one CSS variable name.
   * @returns The value or object, and its options: a number for a value, and an object of
   * numbers for an object. Null and 0 when no token matches.
   */
  #readFromCss(path: readonly string[]): [unknown, unknown] {
    /**
     * Returns Tailwind's options of a token.
     * @param token - Active token.
     * @returns The `DEFAULT` flag for a definition that ordinary definitions override.
     */
    const option = (token: NativeThemeToken | undefined) => (token?.fallback ? DEFAULT_OPTION : 0);
    if (path.length === 1 && path[0].startsWith("--")) {
      const token = this.#tokens.get(path[0]);
      return [token?.value ?? null, option(token)];
    }
    const name = keyPathToCssProperty(path, this.#namespaces);
    if (!name) {
      return [null, 0];
    }
    const values = new Map<string | null, unknown>();
    const options = new Map<string | null, unknown>();
    const nested = new Map<string | null, Record<string, [string, number]>>();
    for (const token of this.#tokens.values()) {
      if (token.name !== `--${name}` && !token.name.startsWith(`--${name}-`)) {
        continue;
      }
      const key = token.name === `--${name}` ? null : token.name.slice(name.length + 3);
      if (key === null || !key.includes("--")) {
        values.set(key, token.value);
        options.set(key, option(token));
        continue;
      }
      const split = key.indexOf("--");
      const main = key.slice(0, split) || null;
      const extra = key
        .slice(split + 2)
        .replace(/-([a-z])/g, (_, letter: string) => letter.toUpperCase());
      nested.set(main, {
        ...nested.get(main),
        [extra]: [token.value, option(token)],
      });
    }
    if (!values.size && !nested.size) {
      return [null, 0];
    }
    for (const [key, extra] of nested) {
      const base = values.get(key);
      if (typeof base === "string") {
        values.set(key, [
          base,
          Object.fromEntries(Object.entries(extra).map(([k, [v]]) => [k, v])),
        ]);
        options.set(key, [
          options.get(key),
          Object.fromEntries(Object.entries(extra).map(([k, [, o]]) => [k, o])),
        ]);
      }
    }
    const result: Dictionary = Object.fromEntries(
      [...values].map(([key, value]) => [key ?? "DEFAULT", value]),
    );
    const resultOptions = Object.fromEntries(
      [...options].map(([key, value]) => [key ?? "DEFAULT", value]),
    );
    if (path.at(-1) === "DEFAULT") {
      return [result.DEFAULT ?? null, resultOptions.DEFAULT ?? 0];
    }
    if ("DEFAULT" in result && Object.keys(result).length === 1) {
      return [result.DEFAULT, resultOptions.DEFAULT ?? 0];
    }
    result.__CSS_VALUES__ = resultOptions;
    return [result, resultOptions];
  }
}
