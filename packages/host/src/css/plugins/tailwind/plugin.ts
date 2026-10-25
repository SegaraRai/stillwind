import type { NativeThemeToken } from "@stillwind/native";
import type { PluginAPI, PluginWithConfig } from "tailwindcss/plugin";

import type { TailwindPlugin } from "../host.ts";
import {
  member,
  probeCss,
  resolveUtility,
  resolveVariant,
  variantBody,
  type DynamicUtility,
  type DynamicVariant,
  type VariantCallback,
  type VariantOptions,
} from "./candidates.ts";
import { configurePlugin, darkModeVariant, resolveConfig } from "./configuration.ts";
import { containerUtility } from "./container.ts";
import {
  cssString,
  identifier,
  selectorRules,
  serializeCss,
  splitTopLevel,
  utilityRules,
} from "./css.ts";
import { PluginTheme, toKeyPath } from "./theme.ts";

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
 * own position in the stylesheet. The project's configuration and the settings that apply to
 * every statement belong to an extra owner, whose CSS {@link TailwindRuntime.settingsCss}
 * returns.
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

  /**
   * Number of `@plugin` statements. It is also the owner index of the project's configuration
   * and of the settings for every statement.
   */
  readonly #owners: number;

  /**
   * `@stillwind-plugin-important` statement for the merged `important` setting, or an empty
   * string when the setting is neither `true` nor a selector.
   */
  readonly #important: string;

  /**
   * @param plugins - Configured handlers, one for each `@plugin` statement, in stylesheet order.
   * @param tokens - Active CSS theme tokens, which the handlers' `theme()` calls read.
   * @param project - Project's Tailwind configuration. Its keys replace the same keys of every
   * plugin configuration, as the keys of a Tailwind `@config` file do. Its plugins register in
   * {@link TailwindRuntime.settingsCss}.
   */
  constructor(
    plugins: readonly PluginWithConfig[],
    tokens: readonly NativeThemeToken[],
    project?: PluginWithConfig["config"],
  ) {
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
        visit(configurePlugin(nested as TailwindPlugin, []), owner);
      }
      entries.push([plugin, owner]);
    };
    for (const [owner, plugin] of plugins.entries()) {
      visit(plugin, owner);
    }
    this.#owners = plugins.length;
    if (project) {
      visit(
        {
          handler: () => {},
          config: project,
        },
        this.#owners,
      );
    }
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
          markerSource: probeCss(options.values, (value) =>
            selectorRules(
              (
                callback as (
                  value: unknown,
                  extra: {
                    modifier: string | null;
                  },
                ) => unknown
              )(value, {
                modifier: null,
              }),
            ),
          ),
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
          markerSource: probeCss((options as VariantOptions).values, (value) => [
            variantBody(
              (callback as VariantCallback)(value, {
                modifier: null,
              }),
            ),
          ]),
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
    // Tailwind defines the `darkMode` variant after running the handlers, so it replaces a
    // plugin's `dark` variant. A stylesheet's `@custom-variant dark` still replaces it, because
    // the native loader gives plugin variants lower precedence.
    const dark = darkModeVariant(config.darkMode);
    if (dark) {
      this.#owner = this.#owners;
      api.addVariant("dark", dark);
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
    if (container) {
      this.#utilities.push({
        owner: this.#owners,
        name: "container",
        css: `@utility container{${container}}`,
      });
    }
  }

  /**
   * Reports whether a compiler discovery request belongs to one of this runtime's callbacks.
   *
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
   *
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
          registration.results.set(candidate, resolveUtility(candidate, registration));
        }
      }
    }
    return before !== this.#all();
  }

  /**
   * Lists finite utility and variant suggestions without executing dynamic callbacks.
   *
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
   *
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
   * @param owner - Index of the statement among the configured handlers, or the number of
   * statements for the settings that {@link TailwindRuntime.settingsCss} describes.
   * @returns Stillwind CSS for the statement's position.
   */
  css(owner: number): string {
    const output =
      owner === this.#owners ? [this.#theme.css(), this.#important].filter(Boolean) : [];
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
      output.push(
        `@stillwind-plugin-family utility ${registration.name};`,
        ...(registration.markerSource
          ? [`@stillwind-marker-source ${cssString(registration.markerSource)};`]
          : []),
      );
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
      output.push(
        `@stillwind-plugin-family variant ${registration.name};`,
        ...(registration.markerSource
          ? [`@stillwind-marker-source ${cssString(registration.markerSource)};`]
          : []),
      );
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

  /**
   * Serializes the settings that apply to every statement, followed by the registrations of the
   * project's configuration, in the form that {@link TailwindRuntime.css} describes.
   *
   * The settings are the merged theme configuration as a `@stillwind-plugin-theme` block, which
   * native utility resolution also reads, the `important` setting, the `dark` variant of the
   * `darkMode` setting, and the `container` utility of `theme.container`.
   *
   * @returns Stillwind CSS for the request that follows the root stylesheet.
   */
  settingsCss(): string {
    return this.css(this.#owners);
  }
}
