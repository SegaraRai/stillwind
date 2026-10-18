/**
 * Adapter that runs plugins written against Tailwind CSS v4's JavaScript plugin API and turns
 * their registrations into Stillwind CSS. The host loads this module only when a `@plugin`
 * statement selects such a plugin, because it depends on the `tailwindcss` package.
 */

import { cssValueMatches, type NativeThemeToken } from "@stillwind/native";
import type { PluginAPI, PluginCreator, PluginWithConfig } from "tailwindcss/plugin";

import type { TailwindPlugin } from "../plugins.ts";
import {
  identifier,
  pluginOptions,
  serializeCss,
  splitTopLevel,
  utilityBodies,
  utilityGlobals,
  utilityRules,
  withAlpha,
} from "./css.ts";
import { merge, PluginTheme } from "./theme.ts";

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
    const config = configs.reduce((result, entry) => merge(result, entry), {
      prefix: "",
      important: false,
    } as Record<string, unknown>);
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
        if (path === "theme" || path.startsWith("theme.")) {
          return this.#theme.get(path.slice(6) || undefined, fallback);
        }
        return (
          path
            .split(".")
            .reduce<unknown>(
              (value, key) =>
                value && typeof value === "object"
                  ? (value as Record<string, unknown>)[key]
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
    const output = this.#globals.filter((entry) => entry.owner === owner).map(({ css }) => css);
    output.push(...this.#utilities.filter((entry) => entry.owner === owner).map(({ css }) => css));
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

/**
 * Decodes an arbitrary value as Tailwind does: `_` becomes a space and `\_` an underscore,
 * except inside `url()`.
 * @param value - Text inside the brackets.
 * @returns The decoded value.
 */
function decode(value: string): string {
  return value.replace(/url\([^)]*\)|\\_|_/g, (part) =>
    part.startsWith("url(") ? part : part === "\\_" ? "_" : " ",
  );
}

/**
 * Reads an arbitrary value or a CSS variable shorthand.
 * @param value - Value or modifier text, such as `[3px]` or `(--size)`.
 * @returns The decoded value, `var(--size)` for `(--size)`, or undefined for a named value.
 */
function arbitrary(value: string): string | undefined {
  if (value.startsWith("[") && value.endsWith("]")) {
    return decode(value.slice(1, -1));
  }
  if (value.startsWith("(") && value.endsWith(")")) {
    return `var(${value.slice(1, -1)})`;
  }
  return undefined;
}

/**
 * Runs a `matchUtilities` callback for one candidate as Tailwind's compatibility layer does.
 *
 * @param candidate - Utility key, such as `-probe-[3px]/mirror`.
 * @param registration - Registration whose root, callback, and options apply.
 * @param global - Receives `@keyframes` and `@property` rules from the callback's result.
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
  const [stem, modifierName] = splitTopLevel(raw, "/");
  const capture = stem === name ? "DEFAULT" : stem.slice(name.length + 1);
  const types = Array.isArray(options.type) ? options.type : [options.type ?? "any"];
  const values = options.values ?? {};
  const isColor = types.includes("color");
  let value: unknown;
  let ignoreModifier = false;
  let modifier: string | null = null;
  const decoded = arbitrary(capture);
  if (decoded !== undefined) {
    const hint = /^([a-z-]+):(.+)$/s.exec(decoded);
    if (hint) {
      if (!types.includes(hint[1]) && !types.includes("any")) {
        return;
      }
      value = hint[2];
    } else {
      if (
        types.length &&
        !types.includes("any") &&
        !types.some((type) => matchesType(decoded, type))
      ) {
        return;
      }
      value = decoded;
    }
  } else {
    if (modifierName && Object.hasOwn(values, `${capture}/${modifierName}`)) {
      value = values[`${capture}/${modifierName}`];
      ignoreModifier = true;
    } else {
      value = values[capture];
    }
    // Color utilities accept these keywords unless their values define the keys, as in Tailwind.
    if (value === undefined && isColor) {
      value = (
        {
          inherit: "inherit",
          transparent: "transparent",
          current: "currentcolor",
        } as Record<string, string>
      )[capture];
    }
    if (value === undefined && values.__BARE_VALUE__ && capture !== "DEFAULT") {
      value = values.__BARE_VALUE__({
        kind: "named",
        value: capture,
        fraction: modifierName ? `${capture}/${modifierName}` : null,
      });
      ignoreModifier = !!modifierName && typeof value === "string" && value.includes("/");
    }
  }
  if (typeof value !== "string" && typeof value !== "number") {
    return;
  }
  if (modifierName) {
    modifier =
      (modifierName.startsWith("[") && modifierName.endsWith("]")
        ? modifierName.slice(1, -1)
        : arbitrary(modifierName)) ??
      (options.modifiers === "any" ? modifierName : options.modifiers?.[modifierName]) ??
      null;
    if (isColor && modifier === null && /^\d+(?:\.\d+)?$/.test(modifierName)) {
      modifier = `${modifierName}%`;
    }
    if (modifier === null && !ignoreModifier) {
      return decoded === undefined ? undefined : null;
    }
    if (isColor && modifier !== null) {
      value = withAlpha(String(value), modifier);
    }
  }
  if (negative) {
    value = `calc(${String(value)} * -1)`;
  }
  return utilityBodies(
    utilityGlobals(
      callback(String(value), {
        modifier,
      }),
      global,
    ),
  );
}

/**
 * Infers whether an arbitrary value without a type hint has a `matchUtilities` type, as
 * Tailwind's `inferDataType()` does.
 * @param value - Decoded arbitrary value.
 * @param type - Tailwind data type, such as `color`, `length`, or `generic-name`.
 * @returns Whether the value has the type.
 */
function matchesType(value: string, type: string): boolean {
  // Tailwind does not infer a type for a variable, so a typed registration skips it.
  if (value.startsWith("var(")) {
    return false;
  }
  // Tailwind's `isGenericName()` list of generic font families.
  if (type === "generic-name") {
    return /^(?:serif|sans-serif|monospace|cursive|fantasy|system-ui|ui-serif|ui-sans-serif|ui-monospace|ui-rounded|math|emoji|fangsong)$/.test(
      value,
    );
  }
  // Tailwind separates percentages from lengths and does not count `none` as an image, while
  // the CSS grammar of Stillwind's value types accepts both.
  if (type === "length" && value.endsWith("%")) {
    return false;
  }
  if (type === "image" && value === "none") {
    return false;
  }
  // Tailwind's `size` type is the grammar of `background-size`.
  return cssValueMatches(value, type === "size" ? "bg-size" : type);
}

/**
 * Selects the callback arguments of a `matchVariant()` registration for one variant name.
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
  const [stem, modifierName] = splitTopLevel(candidate, "/");
  const capture = stem === name ? "DEFAULT" : stem.slice(name.length + 1);
  const value = arbitrary(capture) ?? options.values?.[capture];
  if (value === undefined) {
    return;
  }
  return {
    name: candidate,
    value,
    modifier: modifierName ? (arbitrary(modifierName) ?? modifierName) : null,
  };
}
