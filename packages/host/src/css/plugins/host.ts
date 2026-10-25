import type { StillwindPlugin, TailwindConfig } from "@stillwind/config";
import type { NativeCssRequest } from "@stillwind/native";

import type { PluginPathHost } from "./paths.ts";

/**
 * A plugin written against Tailwind CSS v4's JavaScript plugin API: a handler, `{ handler,
 * config }`, or a factory from `plugin.withOptions`. The host loads its adapter, and with it the
 * `tailwindcss` package, only when a `@plugin` statement selects such a plugin.
 */
export type TailwindPlugin =
  | ((...args: never[]) => unknown)
  | {
      /** Handler that receives Tailwind's plugin API. */
      handler: (...args: never[]) => unknown;
      /** v3-style config, such as `theme.extend`. */
      config?: object;
    };

/**
 * Project settings that the Tailwind plugin adapter applies to the whole CSS graph.
 */
export interface TailwindSettings {
  /** Tailwind configuration, or a promise for one, which replaces Tailwind's `@config`. */
  tailwind?: TailwindConfig | Promise<TailwindConfig>;
  /** Value of Tailwind's `important` configuration key. */
  important?: true | string;
}

/**
 * A native Stillwind plugin or a plugin using Tailwind v4's JavaScript API.
 */
export type LoadedPlugin = StillwindPlugin | TailwindPlugin;

/**
 * Recognizes Tailwind's plugin export forms without executing a factory.
 *
 * @param value - Default export of a plugin module.
 * @returns Whether `value` is a function or an object with a `handler` function.
 */
function isTailwindPlugin(value: unknown): value is TailwindPlugin {
  return (
    typeof value === "function" ||
    (typeof value === "object" &&
      value !== null &&
      typeof (
        value as {
          handler?: unknown;
        }
      ).handler === "function")
  );
}

/**
 * Whether a loaded plugin uses the Stillwind plugin contract.
 *
 * @param plugin - Configured or loaded plugin.
 * @returns Whether `plugin` has a `css` function, which Tailwind plugins lack.
 */
export function nativePlugin(plugin: LoadedPlugin): plugin is StillwindPlugin {
  return (
    typeof plugin === "object" &&
    plugin !== null &&
    "css" in plugin &&
    typeof plugin.css === "function"
  );
}

/**
 * Host operations for plugin packages and the files that plugins read. Paths are absolute host
 * identities, which the host's `read` accepts.
 */
export interface PluginHost extends PluginPathHost {
  /**
   * Loads the plugin package that an `@plugin` statement names when no configured plugin has
   * that name.
   *
   * @param specifier - Package specifier or path from the statement.
   * @param importer - Resolved identity of the stylesheet that contains the statement.
   * @param watch - Registers each file whose change must load the plugin again. A host that
   * returns the same plugin for unchanged files still registers them on every call.
   * @returns The plugin that the module exports by default, as {@link defaultPluginExport} checks.
   * @throws If resolution or evaluation fails, or the default export is not a plugin.
   */
  load(specifier: string, importer: string, watch: (path: string) => void): Promise<LoadedPlugin>;

  /**
   * Reads a text file requested by a plugin.
   *
   * @param path - Absolute file path.
   * @returns The file's text.
   * @throws If the file does not exist or cannot be read.
   */
  read(path: string): Promise<string>;
}

/**
 * Selects the plugin that a module exports by default.
 *
 * @param module - Evaluated module namespace.
 * @param specifier - Request that loaded the module, used in the error message.
 * @returns The default export.
 * @throws If the default export is neither a Stillwind plugin object nor a Tailwind v4 plugin.
 */
export function defaultPluginExport(module: unknown, specifier: string): LoadedPlugin {
  const plugin = (module as { default?: unknown } | null)?.default;
  if (isTailwindPlugin(plugin)) {
    return plugin;
  }
  if (
    typeof plugin !== "object" ||
    plugin === null ||
    typeof (plugin as StillwindPlugin).name !== "string" ||
    typeof (plugin as StillwindPlugin).css !== "function"
  ) {
    throw new Error(
      `Stillwind plugin ${specifier} must export a plugin object by default, or a Tailwind v4 handler`,
    );
  }
  return plugin as StillwindPlugin;
}

/**
 * Returns the utility text before the cursor for plugin catalog lookup.
 *
 * @param text - Text before the cursor. The candidate ends at the cursor and starts after the
 * last whitespace, quote, backtick, `;`, `{`, or `}`.
 * @returns The candidate without its variants, such as `icon-mdi--cl` for `hover:icon-mdi--cl`.
 */
export function catalogInput(text: string): string {
  const candidate = /[^\s"'`;{}]*$/.exec(text)![0];
  let depth = 0;
  for (let index = candidate.length - 1; index >= 0; index--) {
    const character = candidate[index];
    if (character === "]" || character === ")") {
      depth++;
    } else if ((character === "[" || character === "(") && depth > 0) {
      depth--;
    } else if (character === ":" && depth === 0) {
      return candidate.slice(index + 1);
    }
  }
  return candidate;
}

/**
 * Identifies one `@plugin` statement independently of its request ID.
 *
 * @param request - Plugin request.
 * @returns Importer, specifier, and JSON options joined by NUL characters.
 */
export function instanceKey(request: NativeCssRequest): string {
  return `${request.importer}\0${request.specifier}\0${JSON.stringify(request.pluginOptions ?? [])}`;
}

/**
 * Identifies one request independently of its ID, which changes between loads.
 *
 * @param request - Loader request.
 * @returns Importer, specifier, and JSON plugin options joined by NUL characters. An `@import`
 * has no options part.
 */
export function answerKey(request: NativeCssRequest): string {
  return `${request.importer}\0${request.specifier}\0${JSON.stringify(request.pluginOptions) ?? ""}`;
}
