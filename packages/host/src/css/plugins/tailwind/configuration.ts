import type { NativePluginOption } from "@stillwind/native";
import type { PluginCreator, PluginWithConfig } from "tailwindcss/plugin";

import type { TailwindPlugin } from "../host.ts";
import { pluginOptions } from "./css.ts";

/**
 * Normalizes a Tailwind plugin export to `{ handler, config }` and applies the options of its
 * `@plugin` statement.
 *
 * @param plugin - Handler, `{ handler, config }`, or factory from `plugin.withOptions()`.
 * @param options - Declarations of the statement's options block, empty without a block.
 * @returns The factory's result for converted `options`, or the plugin in object form.
 * @throws If a handler that is not a factory receives options, or an option value is invalid.
 */
export function configurePlugin(
  plugin: TailwindPlugin,
  options: readonly NativePluginOption[],
): PluginWithConfig {
  if (
    typeof plugin === "function" &&
    "__isOptionsFunction" in plugin &&
    plugin.__isOptionsFunction
  ) {
    return (plugin as unknown as (options: Record<string, unknown>) => PluginWithConfig)(
      pluginOptions(options),
    );
  }
  if (options.length) {
    throw new Error("This Tailwind plugin does not accept options");
  }
  return typeof plugin === "function"
    ? {
        handler: plugin as PluginCreator,
      }
    : (plugin as PluginWithConfig);
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
export function resolveConfig(
  configs: readonly Record<string, unknown>[],
): Record<string, unknown> {
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
 * Returns the `dark` variant that Tailwind's `darkMode` configuration key defines.
 *
 * @param setting - `"media"`, `"selector"`, `"class"`, or a tuple of one of these modes and a
 * selector, which defaults to `.dark`. `["variant", selectors]` uses a selector or a list of
 * selectors, and each must contain `&`.
 * @returns The value for `addVariant()`, or undefined when the setting defines no variant,
 * including `null`, `false`, and the `variant` mode with a selector that Tailwind rejects.
 */
export function darkModeVariant(setting: unknown): string | string[] | undefined {
  const [mode, selector = ".dark"] = Array.isArray(setting) ? setting : [setting];
  switch (mode) {
    case "media":
      return "@media (prefers-color-scheme: dark)";

    case "selector":
      return `&:where(${selector}, ${selector} *)`;

    case "class":
      return `&:is(${selector} *)`;

    case "variant": {
      const selectors: unknown[] = Array.isArray(selector) ? selector : [selector];
      // Tailwind warns and defines no variant for the default selector or a selector without `&`.
      return selectors.every(
        (entry): entry is string =>
          typeof entry === "string" && entry !== ".dark" && entry.includes("&"),
      )
        ? (selectors as string[])
        : undefined;
    }

    default:
      return undefined;
  }
}
