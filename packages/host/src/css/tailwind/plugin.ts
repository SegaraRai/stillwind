/**
 * Adapter that runs plugins written against Tailwind CSS v4's JavaScript plugin API and turns
 * their registrations into Stillwind CSS. The host loads this module only when a `@plugin`
 * statement selects such a plugin, because it depends on the `tailwindcss` package.
 */

import { decodeArbitraryValue, type NativeThemeToken } from "@stillwind/native";
import type { PluginAPI, PluginCreator, PluginWithConfig } from "tailwindcss/plugin";

import type { TailwindPlugin } from "../plugins.ts";
import { containerUtility } from "./container.ts";
import {
  identifier,
  pluginOptions,
  serializeCss,
  splitTopLevel,
  utilityBodies,
  utilityProperties,
  utilityRules,
  withAlpha,
} from "./css.ts";
import { inferDataType, type DataType } from "./data-type.ts";
import { PluginTheme, toKeyPath } from "./theme.ts";

/**
 * Normalizes a Tailwind plugin export to `{ handler, config }` and applies the options of its
 * `@plugin` statement.
 *
 * @param plugin - Handler, `{ handler, config }`, or factory from `plugin.withOptions()`.
 * @param options - Options block text of the statement, empty without a block.
 * @returns The factory's result for parsed `options`, or the plugin in object form.
 * @throws If a handler that is not a factory receives options, or an option value is invalid.
 */
export function configurePlugin(plugin: TailwindPlugin, options: string): PluginWithConfig {
  if (
    typeof plugin === "function" &&
    "__isOptionsFunction" in plugin &&
    plugin.__isOptionsFunction
  ) {
    return (plugin as unknown as (options: Record<string, unknown>) => PluginWithConfig)(
      pluginOptions(options),
    );
  }
  if (options.trim()) {
    throw new Error("This Tailwind plugin does not accept options");
  }
  return typeof plugin === "function"
    ? {
        handler: plugin as PluginCreator,
      }
    : (plugin as PluginWithConfig);
}

/** Options of one `matchUtilities()` call. */
type MatchOptions = NonNullable<Parameters<PluginAPI["matchUtilities"]>[1]>;

/** Callback of one utility root in `matchUtilities()`. */
type UtilityCallback = Parameters<PluginAPI["matchUtilities"]>[0][string];

/** Callback of `matchVariant()`, which returns a selector, an at-rule, or a list of them. */
type VariantCallback = (
  value: unknown,
  extra: {
    modifier: string | null;
  },
) => string | string[];

/** Options of one `matchVariant()` call. */
interface VariantOptions {
  /** Named values, with `DEFAULT` for the bare root. */
  values?: Record<string, unknown>;
  /** Orders the generated variants; without it, the order of `values` decides. */
  sort?: (
    a: {
      value: unknown;
      modifier: string | null;
    },
    b: {
      value: unknown;
      modifier: string | null;
    },
  ) => number;
}

/** One root of a `matchUtilities()` call and its results so far. */
interface DynamicUtility {
  /** Index of the `@plugin` statement whose handler registered the utility. */
  owner: number;
  /** Utility root, such as `scrollbar`. */
  name: string;
  callback: UtilityCallback;
  options: MatchOptions;
  /** Generated bodies by request, null when the callback stops evaluation. */
  results: Map<string, string[] | null | undefined>;
}

/** One variant that a `matchVariant()` callback generated. */
interface VariantResult {
  /** Complete variant name, such as `state-[open]`. */
  name: string;
  /** Value passed to the callback. */
  value: unknown;
  /** Modifier passed to the callback. */
  modifier: string | null;
  /** Variant body with `@slot`. */
  body: string;
}

/** One `matchVariant()` registration and its results so far. */
interface DynamicVariant {
  /** Index of the `@plugin` statement whose handler registered the variant. */
  owner: number;
  /** Variant root, such as `state`. */
  name: string;
  callback: VariantCallback;
  options: VariantOptions;
  /** Generated variants by complete name. */
  results: Map<string, VariantResult>;
}

/** A static registration and the `@plugin` statement that registered it. */
interface Owned {
  /** Index of the `@plugin` statement whose handler registered the CSS. */
  owner: number;
  /** Registered utility or variant name, without escapes, when the CSS registers one. */
  name?: string;
  /** Stillwind CSS of the registration. */
  css: string;
}

/**
 * Builds the part of Tailwind's resolved config that `config()` reads besides `theme`.
 *
 * Like Tailwind, the result keeps only `darkMode`, `prefix`, `blocklist`, and `important` from
 * the configs, so `config("custom")` returns the fallback. The last config that defines one of
 * these keys sets it, and a `null` value restores the default.
 *
 * @param configs - Plugin configs in registration order, presets first.
 * @returns The resolved keys with Tailwind's defaults for the others.
 */
function resolveConfig(configs: readonly Record<string, unknown>[]): Record<string, unknown> {
  const defaults: Record<string, unknown> = {
    darkMode: null,
    prefix: "",
    blocklist: [],
    important: false,
  };
  const result: Record<string, unknown> = {
    ...defaults,
    future: {},
    experimental: {},
    plugins: [],
    content: {
      files: [],
    },
  };
  for (const config of configs) {
    for (const [key, fallback] of Object.entries(defaults)) {
      if (config[key] !== undefined) {
        result[key] = config[key] ?? fallback;
      }
    }
  }
  return result;
}

/**
 * Executes Tailwind API handlers once and resolves their registered callbacks on demand.
 *
 * All handlers share one resolved config and theme, as in Tailwind. Each registration keeps the
 * index of the `@plugin` statement that registered it, so the CSS of each statement stays at its
 * own position in the stylesheet.
 */
export class TailwindRuntime {
  /** `addUtilities()` and `addComponents()` registrations, as `@utility` rules. */
  readonly #utilities: Owned[] = [];
  /** `addVariant()` registrations by name, as `@custom-variant` rules. */
  readonly #variants = new Map<string, Owned>();
  /** Base styles, keyframes, and `@property` rules. */
  readonly #globals: Owned[] = [];
  readonly #dynamicUtilities: DynamicUtility[] = [];
  readonly #dynamicVariants: DynamicVariant[] = [];
  /** Requests already resolved, including ones that no registration accepted. */
  readonly #requests = new Set<string>();
  /** Theme shared by every handler. */
  readonly #theme: PluginTheme;

  /** Owner whose handler is running, recorded with each registration. */
  #owner = 0;
  /** Number of `@plugin` statements, which are the owners. */
  readonly #owners: number;
  /**
   * `@stillwind-plugin-important` statement for the merged `important` setting, or an empty
   * string when the setting is neither `true` nor a selector.
   */
  readonly #important: string;

  /**
   * @param plugins - Configured handlers, one for each `@plugin` statement, in stylesheet order.
   * @param tokens - Active CSS theme tokens, which the handlers' `theme()` calls read.
   */
  constructor(plugins: readonly PluginWithConfig[], tokens: readonly NativeThemeToken[]) {
    const entries: [PluginWithConfig, number][] = [];
    const configs: Record<string, unknown>[] = [];
    /**
     * Collects a plugin's presets, config, and nested plugins before the plugin itself.
     * @param plugin - Configured plugin.
     * @param owner - Statement whose output receives the plugin's registrations.
     */
    const visit = (plugin: PluginWithConfig, owner: number) => {
      for (const preset of plugin.config?.presets ?? []) {
        visit(
          {
            handler: () => {},
            config: preset,
          },
          owner,
        );
      }
      configs.push((plugin.config ?? {}) as Record<string, unknown>);
      for (const nested of plugin.config?.plugins ?? []) {
        visit(configurePlugin(nested as TailwindPlugin, ""), owner);
      }
      entries.push([plugin, owner]);
    };
    for (const [owner, plugin] of plugins.entries()) {
      visit(plugin, owner);
    }
    this.#owners = plugins.length;
    this.#theme = new PluginTheme(configs, tokens);
    const config = resolveConfig(configs);
    const addUtilities: PluginAPI["addUtilities"] = (css) =>
      utilityRules(
        css,
        (name, body) =>
          this.#utilities.push({
            owner: this.#owner,
            name,
            css: `@utility ${identifier(name)}{${body}}`,
          }),
        (body) =>
          this.#globals.push({
            owner: this.#owner,
            css: body,
          }),
      );
    const matchUtilities: PluginAPI["matchUtilities"] = (utilities, options = {}) => {
      for (const [name, callback] of Object.entries(utilities)) {
        this.#dynamicUtilities.push({
          owner: this.#owner,
          name,
          callback,
          options,
          results: new Map(),
        });
      }
    };
    const api: PluginAPI = {
      addBase: (css) => {
        this.#globals.push({
          owner: this.#owner,
          css: `@layer base{${serializeCss(css)}}`,
        });
      },
      addUtilities,
      addComponents: addUtilities,
      matchUtilities,
      matchComponents: matchUtilities,
      addVariant: (name, value) => {
        const body = variantBody(value);
        // Tailwind v4 drops the v3 `:merge()` form, which these plugins also provide.
        if (!body.includes(":merge(")) {
          // A later registration replaces an earlier one and keeps its position, as in Tailwind.
          const previous = this.#variants.get(name);
          this.#variants.set(name, {
            owner: previous?.owner ?? this.#owner,
            css: `@custom-variant ${identifier(name)}{${body}}`,
          });
        }
      },
      matchVariant: (name, callback, options = {}) => {
        try {
          if (
            String(
              callback("a", {
                modifier: null,
              }),
            ).includes(":merge(")
          ) {
            return;
          }
        } catch {}
        this.#dynamicVariants.push({
          owner: this.#owner,
          name,
          callback: callback as VariantCallback,
          options: options as VariantOptions,
          results: new Map(),
        });
      },
      theme: this.#theme.get,
      config: (path, fallback) => {
        if (!path) {
          return {
            ...config,
            theme: this.#theme.get(),
          };
        }
        // Tailwind reads a config path with `toKeyPath()`, so `theme[colors][brand]` and
        // `theme.colors.brand` name the same value.
        const [key, ...keys] = toKeyPath(path);
        if (key === "theme") {
          return keys.length
            ? this.#theme.get(keys.map((entry) => `[${entry}]`).join(""), fallback)
            : this.#theme.get();
        }
        return (
          [key, ...keys].reduce<unknown>(
            (value, entry) =>
              value && typeof value === "object"
                ? (value as Record<string, unknown>)[entry]
                : undefined,
            config,
          ) ?? fallback
        );
      },
      prefix: (className) => className,
    };
    for (const [plugin, owner] of entries) {
      this.#owner = owner;
      plugin.handler(api);
    }
    // `config.prefix` is not applied: Stillwind generates unique class names, so it does not
    // prefix candidates.
    const { important } = config;
    this.#important =
      important === true
        ? "@stillwind-plugin-important;"
        : typeof important === "string" && important.trim()
          ? `@stillwind-plugin-important ${important.trim()};`
          : "";
    const screens = this.#theme.get("screens");
    const container = containerUtility(
      this.#theme.get("container"),
      Object.fromEntries(
        Object.entries(screens && typeof screens === "object" ? screens : {}).filter(
          (entry): entry is [string, string] =>
            entry[0] !== "__CSS_VALUES__" && typeof entry[1] === "string",
        ),
      ),
    );
    if (container && this.#owners) {
      this.#utilities.push({
        owner: 0,
        name: "container",
        css: `@utility container{${container}}`,
      });
    }
  }

  /**
   * Returns the plugins' theme configuration as CSS, which native utility resolution also reads.
   * @returns A `@stillwind-plugin-theme` block, or an empty string without configuration.
   */
  themeCss(): string {
    return this.#theme.css();
  }

  /**
   * Reports whether a compiler discovery request belongs to one of this runtime's callbacks.
   * @param candidate - Utility key, or variant name with an `@variant ` prefix.
   * @returns Whether a `matchUtilities()` or `matchVariant()` root matches it.
   */
  accepts(candidate: string): boolean {
    if (candidate.startsWith("@variant ")) {
      return this.#dynamicVariants.some(({ name }) => member(candidate.slice(9), name));
    }
    const value = candidate.replace(/^-/, "");
    return this.#dynamicUtilities.some(({ name }) => member(value, name));
  }

  /**
   * Runs the callbacks for requests that this runtime has not seen.
   * @param candidates - Utility keys, and variant names with an `@variant ` prefix.
   * @returns Whether the generated CSS of any statement changed.
   * @throws If a callback fails.
   */
  resolve(candidates: readonly string[]): boolean {
    const before = this.#all();
    for (const candidate of candidates) {
      if (this.#requests.has(candidate)) {
        continue;
      }
      this.#requests.add(candidate);
      if (candidate.startsWith("@variant ")) {
        for (const registration of this.#dynamicVariants) {
          const variant = resolveVariant(candidate.slice(9), registration);
          if (variant) {
            registration.results.set(variant.name, {
              ...variant,
              body: variantBody(
                registration.callback(variant.value, {
                  modifier: variant.modifier,
                }),
              ),
            });
          }
        }
      } else {
        for (const registration of this.#dynamicUtilities) {
          registration.results.set(
            candidate,
            resolveUtility(candidate, registration, (css) =>
              this.#globals.push({
                owner: registration.owner,
                css,
              }),
            ),
          );
        }
      }
    }
    return before !== this.#all();
  }

  /**
   * Lists finite utility and variant suggestions without executing dynamic callbacks.
   * @returns Static utility names, the named values of dynamic utilities with their modifiers and
   * negative forms, and variant names with a trailing `:`.
   */
  catalog(): string[] {
    const result = [
      ...new Set(this.#utilities.map(({ name }) => name!)),
      ...[...this.#variants.keys()].map((name) => `${name}:`),
    ];
    for (const { name, options } of this.#dynamicUtilities) {
      const keys = Object.keys(options.values ?? {}).filter((key) => !key.startsWith("__"));
      const modifiers =
        options.modifiers && options.modifiers !== "any" ? Object.keys(options.modifiers) : [];
      for (const key of keys) {
        const candidate = key === "DEFAULT" ? name : `${name}-${key}`;
        result.push(candidate, ...modifiers.map((modifier) => `${candidate}/${modifier}`));
        if (options.supportsNegativeValues) {
          result.push(`-${candidate}`);
        }
      }
    }
    for (const { name, options } of this.#dynamicVariants) {
      for (const key of Object.keys(options.values ?? {})) {
        result.push(`${key === "DEFAULT" ? name : `${name}-${key}`}:`);
      }
    }
    return result;
  }

  /**
   * Serializes every statement, for change detection.
   * @returns The CSS of all statements in order.
   */
  #all(): string {
    return Array.from({ length: this.#owners }, (_, owner) => this.css(owner)).join("\n");
  }

  /**
   * Serializes the registrations of one `@plugin` statement in handler order, including sorted
   * dynamic variants.
   *
   * A static utility becomes an ordinary `@utility`. A callback result for one candidate becomes
   * a `(functional)` `@utility`, so it adds to the built-in and stylesheet definitions of its
   * root, as Tailwind evaluates every definition of a name. A registration whose types include
   * `any` and another type is a Tailwind fallback, and a callback that rejects an arbitrary value
   * with an invalid modifier stops the evaluation of later definitions.
   *
   * @param owner - Index of the statement among the configured handlers.
   * @returns Stillwind CSS for the statement's position.
   */
  css(owner: number): string {
    const output = owner === 0 && this.#important ? [this.#important] : [];
    output.push(
      ...[...this.#globals, ...this.#utilities]
        .filter((entry) => entry.owner === owner)
        .map(({ css }) => css),
    );
    for (const variant of this.#variants.values()) {
      if (variant.owner === owner) {
        output.push(variant.css);
      }
    }
    for (const registration of this.#dynamicUtilities) {
      if (registration.owner !== owner) {
        continue;
      }
      output.push(`@stillwind-plugin-family utility ${registration.name};`);
      const types = Array.isArray(registration.options.type)
        ? registration.options.type
        : [registration.options.type ?? "any"];
      const role =
        types.length > 1 && types.includes("any") ? "functional, fallback" : "functional";
      for (const [request, body] of registration.results) {
        if (body === null) {
          output.push(`@utility ${identifier(request)} (functional, stop){}`);
        }
        for (const target of body ?? []) {
          output.push(`@utility ${identifier(request)} (${role}){${target}}`);
        }
      }
    }
    for (const registration of this.#dynamicVariants) {
      if (registration.owner !== owner) {
        continue;
      }
      output.push(`@stillwind-plugin-family variant ${registration.name};`);
      const candidates = [...registration.results.values()];
      candidates.sort((a, b) => {
        if (registration.options.sort) {
          return registration.options.sort(a, b);
        }
        const keys = Object.keys(registration.options.values ?? {});
        /**
         * Positions a variant by its value's key in `values`.
         * @param candidate - Generated variant.
         * @returns The key's index, or the number of keys for an arbitrary value.
         */
        const order = (candidate: typeof a) => {
          const [stem] = splitTopLevel(candidate.name, "/");
          const key =
            stem === registration.name ? "DEFAULT" : stem.slice(registration.name.length + 1);
          const index = keys.indexOf(key);
          return index < 0 ? keys.length : index;
        };
        return order(a) - order(b) || (String(a.value) < String(b.value) ? -1 : 1);
      });
      for (const candidate of candidates) {
        output.push(`@custom-variant ${identifier(candidate.name)}{${candidate.body}}`);
      }
    }
    return output.join("\n");
  }
}

/**
 * Whether a candidate or variant belongs to a registered root.
 * @param candidate - Name without a leading `-`.
 * @param root - Registered root, such as `scrollbar`.
 * @returns Whether the name is the root or continues it with `-` or `/`.
 */
function member(candidate: string, root: string): boolean {
  return candidate === root || candidate.startsWith(`${root}-`) || candidate.startsWith(`${root}/`);
}

/**
 * Converts a variant definition that a plugin returns into a `@custom-variant` body.
 * @param value - Selector or at-rule string, CSS object, or a list of them.
 * @returns CSS with `@slot`. A string without `@slot` gets one at its innermost block, or a
 * block of its own.
 */
function variantBody(value: unknown): string {
  if (Array.isArray(value)) {
    return value.map(variantBody).join("\n");
  }
  if (typeof value !== "string") {
    return serializeCss(value);
  }
  if (value.includes("@slot")) {
    return value;
  }
  if (value.includes("{")) {
    return value.replace("}", "{@slot;}}");
  }
  return `${value}{@slot;}`;
}

/** A candidate value or modifier, as Tailwind's candidate parser reads it. */
type CandidatePart =
  | {
      kind: "named";
      /** Text as written. */
      value: string;
    }
  | {
      kind: "arbitrary";
      /** Decoded value; a `(--name)` shorthand becomes `var(--name)`. */
      value: string;
      /** Explicit type hint such as `length` in `[length:1px]`, or null without one. */
      dataType: string | null;
    };

/**
 * Whether a decoded arbitrary value has no top-level `;` or `}`, which Tailwind rejects.
 * @param value - Decoded value.
 * @returns Whether the value can be used.
 */
function validArbitrary(value: string): boolean {
  return splitTopLevel(value, ";").length === 1 && splitTopLevel(value, "}").length === 1;
}

/**
 * Parses the value part of a candidate as Tailwind's candidate parser does.
 * @param text - Text after the root and its `-`, such as `4`, `[length:1px]`, or
 * `(length:--size)`.
 * @returns The value, or undefined when Tailwind rejects the candidate.
 */
function parseValue(text: string): CandidatePart | undefined {
  let inner: string;
  if (text.startsWith("(") && text.endsWith(")")) {
    // Tailwind rewrites `(type:--name)` to `[type:var(--name)]` before decoding.
    const parts = splitTopLevel(text.slice(1, -1), ":");
    const [dataType, variable] = parts.length === 2 ? parts : [null, parts.join(":")];
    if (!variable.startsWith("--") || !validArbitrary(variable)) {
      return undefined;
    }
    inner = dataType === null ? `var(${variable})` : `${dataType}:var(${variable})`;
  } else if (text.startsWith("[") && text.endsWith("]")) {
    inner = text.slice(1, -1);
  } else {
    return {
      kind: "named",
      value: text,
    };
  }
  let value = decodeArbitraryValue(inner);
  if (!validArbitrary(value)) {
    return undefined;
  }
  // A type hint consists of lowercase letters and hyphens before the first colon.
  let dataType: string | null = null;
  const hint = /^([a-z-]*):/.exec(value);
  if (hint) {
    if (!hint[1]) {
      return undefined;
    }
    dataType = hint[1];
    value = value.slice(hint[0].length);
  }
  return value.trim()
    ? {
        kind: "arbitrary",
        value,
        dataType,
      }
    : undefined;
}

/**
 * Parses a `/` modifier as Tailwind's candidate parser does.
 * @param text - Modifier text without the slash.
 * @returns The modifier, or undefined when Tailwind rejects the candidate.
 */
function parseModifier(text: string): CandidatePart | undefined {
  if (text.startsWith("[") && text.endsWith("]")) {
    const value = decodeArbitraryValue(text.slice(1, -1));
    return validArbitrary(value) && value.trim()
      ? {
          kind: "arbitrary",
          value,
          dataType: null,
        }
      : undefined;
  }
  if (text.startsWith("(") && text.endsWith(")")) {
    const variable = text.slice(1, -1);
    return variable.startsWith("--") && validArbitrary(variable)
      ? {
          kind: "arbitrary",
          value: decodeArbitraryValue(`var(${variable})`),
          dataType: null,
        }
      : undefined;
  }
  return text
    ? {
        kind: "named",
        value: text,
      }
    : undefined;
}

/**
 * Runs a `matchUtilities` callback for one candidate as Tailwind's compatibility layer does.
 *
 * @param candidate - Utility key, such as `-probe-[3px]/mirror`.
 * @param registration - Registration whose root, callback, and options apply.
 * @param global - Receives `@property` rules from the callback's result.
 * @returns The bodies of the callback's targets, undefined when the registration does not apply,
 * and null when an arbitrary value has an invalid modifier, which ends Tailwind's evaluation of
 * later definitions.
 */
function resolveUtility(
  candidate: string,
  { name, callback, options }: DynamicUtility,
  global: (css: string) => void,
): string[] | null | undefined {
  const negative = candidate.startsWith("-");
  const raw = negative ? candidate.slice(1) : candidate;
  if (!member(raw, name) || (negative && !options.supportsNegativeValues)) {
    return;
  }
  const [stem, ...modifierParts] = splitTopLevel(raw, "/");
  if (modifierParts.length > 1) {
    return;
  }
  const modifierText = modifierParts[0];
  const valueText = stem === name ? undefined : stem.slice(name.length + 1);
  const parsed = valueText === undefined ? null : parseValue(valueText);
  const candidateModifier = modifierText === undefined ? null : parseModifier(modifierText);
  if (parsed === undefined || candidateModifier === undefined) {
    return;
  }
  const types = Array.isArray(options.type) ? options.type : [options.type ?? "any"];
  if (parsed?.kind === "arbitrary" && types.length > 0 && !types.includes("any")) {
    if (
      parsed.dataType
        ? !types.includes(parsed.dataType)
        : !inferDataType(parsed.value, types as DataType[])
    ) {
      return;
    }
  }
  const isColor = types.includes("color");
  // Color utilities accept these keywords unless their values define the keys, as in Tailwind.
  const values: Record<string, unknown> = isColor
    ? {
        inherit: "inherit",
        transparent: "transparent",
        current: "currentcolor",
        ...options.values,
      }
    : (options.values ?? {});
  // A named value with a named modifier, such as `w-1/2`, may be a fraction key.
  const fraction =
    parsed?.kind === "named" && candidateModifier?.kind === "named"
      ? `${parsed.value}/${candidateModifier.value}`
      : null;
  let value: unknown = null;
  let ignoreModifier = false;
  if (!parsed) {
    value = values.DEFAULT ?? null;
  } else if (parsed.kind === "arbitrary") {
    value = parsed.value;
  } else if (fraction && Object.hasOwn(values, fraction)) {
    value = values[fraction];
    ignoreModifier = true;
  } else if (Object.hasOwn(values, parsed.value)) {
    value = values[parsed.value];
  } else if (typeof values.__BARE_VALUE__ === "function") {
    value =
      values.__BARE_VALUE__({
        kind: "named",
        value: parsed.value,
        fraction,
      }) ?? null;
    ignoreModifier = fraction !== null && typeof value === "string" && value.includes("/");
  }
  if (value === null || value === undefined) {
    return;
  }
  let modifier: string | null = null;
  if (candidateModifier) {
    const modifiers = options.modifiers;
    if (modifiers === "any" || candidateModifier.kind === "arbitrary") {
      modifier = candidateModifier.value;
    } else if (modifiers && Object.hasOwn(modifiers, candidateModifier.value)) {
      modifier = modifiers[candidateModifier.value];
    } else if (isColor && !Number.isNaN(Number(candidateModifier.value))) {
      modifier = `${candidateModifier.value}%`;
    }
    if (modifier === null && !ignoreModifier) {
      return parsed?.kind === "arbitrary" ? null : undefined;
    }
  }
  // Like Tailwind, these interpolate the value as text; color and negative registrations hold
  // strings or numbers.
  if (isColor && modifier !== null) {
    value = withAlpha(String(value as string), modifier);
  }
  if (negative) {
    value = `calc(${String(value as string)} * -1)`;
  }
  return utilityBodies(
    utilityProperties(
      // Tailwind passes a value from `values` as it is, so a number stays a number.
      (
        callback as (
          value: unknown,
          extra: {
            modifier: string | null;
          },
        ) => unknown
      )(value, {
        modifier,
      }),
      global,
    ),
  );
}

/**
 * Selects the callback arguments of a `matchVariant()` registration for one variant name, as
 * Tailwind's compatibility layer does.
 *
 * The bare root uses the `DEFAULT` value. A named value must be a string in `values`, so
 * `values: { one: 1 }` defines no `name-one` variant. An arbitrary value passes its decoded text.
 *
 * @param candidate - Complete variant name, such as `state-[open]/peer`.
 * @param registration - Registration whose root and values apply.
 * @returns The variant name, value, and modifier, or undefined when the registration has no
 * such value.
 */
function resolveVariant(
  candidate: string,
  { name, options }: DynamicVariant,
):
  | {
      name: string;
      value: unknown;
      modifier: string | null;
    }
  | undefined {
  if (!member(candidate, name)) {
    return;
  }
  const [stem, ...modifierParts] = splitTopLevel(candidate, "/");
  if (modifierParts.length > 1) {
    return;
  }
  const valueText = stem === name ? undefined : stem.slice(name.length + 1);
  const parsed = valueText === undefined ? null : parseValue(valueText);
  const modifier = modifierParts[0] === undefined ? null : parseModifier(modifierParts[0]);
  if (parsed === undefined || modifier === undefined) {
    return;
  }
  const values = options.values;
  let value: unknown;
  if (!parsed) {
    if (!values || !("DEFAULT" in values)) {
      return;
    }
    value = values.DEFAULT;
  } else if (parsed.kind === "arbitrary") {
    value = parsed.value;
  } else {
    value = values && Object.hasOwn(values, parsed.value) ? values[parsed.value] : undefined;
    if (typeof value !== "string") {
      return;
    }
  }
  return {
    name: candidate,
    value,
    modifier: modifier?.value ?? null,
  };
}
