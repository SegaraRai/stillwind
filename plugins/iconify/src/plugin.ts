import type { PluginContext, StillwindPlugin } from "@stillwind/config";

import { Collections, type CollectionSource } from "./collections.ts";
import { iconCss, type IconMode, type IconStyle } from "./icons.ts";

/**
 * Spellings that select an icon. `named` accepts `icon-mdi--close`, and `bracket` accepts
 * `icon-[mdi--close]`. `both` accepts both spellings, which then select one utility, and
 * canonical lint suggests the named spelling.
 */
export type IconSyntax = "both" | "named" | "bracket";

/**
 * Options of {@link iconify}. The options block of a `@plugin` statement can set `prefix`,
 * `syntax`, `scale`, and `mode`, and those values replace the options given here.
 */
export interface IconifyOptions {
  /** Utility prefix without the trailing hyphen. Defaults to `icon`. */
  readonly prefix?: string;
  /** Spellings that select an icon. Defaults to `both`. */
  readonly syntax?: IconSyntax;
  /** Icon height in `em`. Defaults to `1`. */
  readonly scale?: number;
  /** Paint method of the icons. Defaults to `auto`. */
  readonly mode?: IconMode;
  /**
   * Icon sets keyed by set prefix. They take precedence over installed `@iconify-json/*` and
   * `@iconify/json` packages.
   */
  readonly collections?: Readonly<Record<string, CollectionSource>>;
}

/** Settings of one `@plugin` statement. */
interface Settings extends IconStyle {
  /** Utility prefix without the trailing hyphen. */
  readonly prefix: string;
  /** Spellings that select an icon. */
  readonly syntax: IconSyntax;
}

/** Iconify set prefixes and icon names: lowercase words joined by single hyphens. */
const ICONIFY_NAME = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;

/** Utility prefixes, which must start a CSS identifier and must not contain `--`. */
const PREFIX = /^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/;

const SYNTAXES: readonly IconSyntax[] = ["both", "named", "bracket"];

const MODES: readonly IconMode[] = ["auto", "mask", "background"];

/**
 * Creates the Iconify plugin, which defines a utility for each icon that modules use.
 *
 * A stylesheet enables it with `@plugin "@stillwind/plugin-iconify";`. A utility names
 * an icon as `<prefix>-<set>--<icon>`, such as `icon-mdi--close`, or as
 * `<prefix>-[<set>--<icon>]`, as the `syntax` option selects. Every instance is named
 * `@stillwind/plugin-iconify`, so an instance in the `plugins` configuration entry
 * replaces the package's default instance in that statement.
 *
 * @param options - Default settings and configured icon sets.
 * @returns A plugin instance. Each loaded statement reads its icon sets again.
 * @throws If an option value is invalid. An invalid value in a statement's options
 * block fails the design-system load instead.
 */
export function iconify(options: IconifyOptions = {}): StillwindPlugin {
  const defaults = settings(options);

  const statements = new WeakMap<PluginContext, Collections>();
  /**
   * Returns the icon sets of one statement.
   * @param context - Statement context. The host keeps it for the lifetime of the loaded statement.
   * @returns The statement's icon set cache.
   */
  const collections = (context: PluginContext) => {
    let found = statements.get(context);
    if (!found) {
      found = new Collections(options.collections ?? {}, context);
      statements.set(context, found);
    }
    return found;
  };

  return {
    name: "@stillwind/plugin-iconify",
    css(context) {
      const { prefix, syntax } = statementSettings(defaults, context);
      return [
        ...(syntax !== "bracket" ? [`@stillwind-dynamic ${prefix}-*;`] : []),
        ...(syntax !== "named" ? [`@stillwind-dynamic ${prefix}-[*];`] : []),
        "",
      ].join("\n");
    },
    async resolve(candidates, context) {
      const current = statementSettings(defaults, context);
      const rules = await Promise.all(
        candidates.map(async (candidate) => {
          const icon = parseCandidate(candidate, current);
          const collection = icon && (await collections(context).get(icon.set));
          return collection
            ? iconCss(
                candidate,
                `--${current.prefix}-${icon.set}--${icon.name}`,
                collection,
                icon.name,
                current,
              )
            : undefined;
        }),
      );
      return rules.filter((rule) => rule !== undefined).join("");
    },
    async catalog(context, input = "") {
      const { prefix, syntax } = statementSettings(defaults, context);
      const store = collections(context);
      const bracket =
        syntax === "bracket" || (syntax === "both" && input.startsWith(`${prefix}-[`));
      const spell = (set: string, name: string) =>
        bracket ? `${prefix}-[${set}--${name}]` : `${prefix}-${set}--${name}`;
      const listed = await store.prefixes();
      const sets = new Set(listed);
      // An input that names a set completes its icons from any source, including `@iconify/json`.
      const typed = typedSet(input, prefix);
      if (typed && !sets.has(typed) && (await store.get(typed))) {
        sets.add(typed);
      }
      const lists = await Promise.all(
        [...sets].map(async (set) => {
          const collection = await store.get(set);
          return collection
            ? [...Object.keys(collection.icons), ...Object.keys(collection.aliases ?? {})].map(
                (name) => spell(set, name),
              )
            : [];
        }),
      );
      // Before an input names a set, the `@iconify/json` sets complete as incomplete names that
      // end with `--`.
      const names = lists.flat();
      if (!typed && input && (`${prefix}-`.startsWith(input) || input.startsWith(`${prefix}-`))) {
        for (const set of await store.bundledPrefixes()) {
          if (!sets.has(set)) {
            names.push(bracket ? `${prefix}-[${set}--` : `${prefix}-${set}--`);
          }
        }
      }
      return names;
    },
  };
}

/**
 * Returns the icon set that an editor input names.
 *
 * @param input - Utility text before the cursor, such as `icon-mdi--cl` or `icon-[mdi--`.
 * @param prefix - Utility prefix without the trailing hyphen.
 * @returns The set prefix, or undefined when the input does not contain `<prefix>-<set>--`.
 */
function typedSet(input: string, prefix: string): string | undefined {
  if (!input.startsWith(`${prefix}-`)) {
    return undefined;
  }
  let rest = input.slice(prefix.length + 1);
  if (rest.startsWith("[")) {
    rest = rest.slice(1);
  }
  const separator = rest.indexOf("--");
  const set = rest.slice(0, separator);
  return separator > 0 && ICONIFY_NAME.test(set) ? set : undefined;
}

/**
 * Splits a candidate into its icon set and icon name.
 *
 * @param candidate - Utility name, such as `icon-mdi--close` or `icon-[mdi--close]`.
 * @param settings - Prefix and the spellings that select an icon.
 * @returns The set and icon names, or undefined when the candidate does not name an icon in
 * an accepted spelling.
 */
function parseCandidate(
  candidate: string,
  { prefix, syntax }: Settings,
): { set: string; name: string } | undefined {
  if (!candidate.startsWith(`${prefix}-`)) {
    return undefined;
  }
  let rest = candidate.slice(prefix.length + 1);
  const bracket = rest.startsWith("[") && rest.endsWith("]");
  if (bracket ? syntax === "named" : syntax === "bracket") {
    return undefined;
  }
  if (bracket) {
    rest = rest.slice(1, -1);
  }
  const separator = rest.indexOf("--");
  const set = rest.slice(0, separator);
  const name = rest.slice(separator + 2);
  return separator > 0 && ICONIFY_NAME.test(set) && ICONIFY_NAME.test(name)
    ? { set, name }
    : undefined;
}

/**
 * Applies a statement's options block to the plugin's defaults.
 *
 * @param defaults - Settings of the plugin instance.
 * @param context - Statement whose options replace the defaults.
 * @returns The settings of the statement.
 * @throws If an option value is invalid.
 */
function statementSettings(defaults: Settings, context: PluginContext): Settings {
  const { prefix, syntax, scale, mode } = context.options;
  return settings({
    prefix: prefix ?? defaults.prefix,
    syntax: (syntax ?? defaults.syntax) as IconSyntax,
    scale: scale === undefined ? defaults.scale : Number(scale),
    mode: (mode ?? defaults.mode) as IconMode,
  });
}

/**
 * Validates options and fills in defaults.
 *
 * @param options - Options that may omit settings.
 * @returns Complete settings.
 * @throws If the prefix is not a lowercase identifier, the syntax or mode is unknown, or the
 * scale is not a positive finite number.
 */
function settings(options: IconifyOptions): Settings {
  const { prefix = "icon", syntax = "both", scale = 1, mode = "auto" } = options;
  if (!PREFIX.test(prefix)) {
    throw new Error(`Invalid Iconify utility prefix: ${prefix}`);
  }
  if (!SYNTAXES.includes(syntax)) {
    throw new Error(`Iconify syntax must be one of ${SYNTAXES.join(", ")}: ${syntax}`);
  }
  if (!Number.isFinite(scale) || scale <= 0) {
    throw new Error(`Iconify scale must be a positive number: ${scale}`);
  }
  if (!MODES.includes(mode)) {
    throw new Error(`Iconify mode must be one of ${MODES.join(", ")}: ${mode}`);
  }
  return {
    prefix,
    syntax,
    scale,
    mode,
  };
}

/** The plugin with default options, which `@plugin "@stillwind/plugin-iconify";` loads. */
const defaultPlugin = iconify();

export default defaultPlugin;
