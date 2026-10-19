/**
 * A Stillwind plugin: JavaScript that generates Stillwind CSS for a design system.
 *
 * A stylesheet loads a plugin with `@plugin "<name>";` or `@plugin "<name>" { option: value; }`.
 * The CSS that {@link StillwindPlugin.css} returns replaces the statement, so it can contain
 * `@utility`, `@custom-variant`, `@theme`, and the other Stillwind directives.
 *
 * A plugin can also define utilities on demand. Its CSS declares a family with
 * `@stillwind-dynamic <prefix>*;` for named candidates such as `icon-mdi--close`, with
 * `@stillwind-dynamic <prefix>[*];` for bracket candidates such as `icon-[mdi--close]`, or with
 * both. The host then passes the candidates of that family that modules use, and that are not
 * defined yet, to {@link StillwindPlugin.resolve}. The returned CSS joins the plugin's CSS, and
 * the host compiles the modules again.
 */
export interface StillwindPlugin {
  /**
   * Name that `@plugin` uses to select this plugin, such as `"@stillwind/plugin-iconify"`.
   * A project's `plugins` entry takes precedence over a package of the same name.
   */
  readonly name: string;
  /**
   * Returns the CSS that replaces the `@plugin` statement.
   *
   * @param context - Options of the statement and host operations.
   * @returns Stillwind CSS. It must not contain `@import` or `@plugin`.
   */
  css(context: PluginContext): string | Promise<string>;
  /**
   * Defines candidates of the dynamic families that the plugin's CSS declares.
   *
   * @param candidates - Utility names without variants, modifiers, or `!`, such as
   * `icon-mdi--close`. When the family accepts both forms, a bracket spelling such as
   * `icon-[mdi--close]` arrives in its named form. When the family accepts only the bracket form,
   * the bracket spelling arrives as written, and the plugin defines it under that name, such as
   * `@utility icon-[mdi--close]`.
   * @param context - Options of the statement and host operations.
   * @returns Stillwind CSS that defines some or all of the candidates. A candidate that it leaves
   * undefined is not passed again while the design system is loaded.
   */
  resolve?(candidates: readonly string[], context: PluginContext): string | Promise<string>;
  /**
   * Lists the candidates that editors complete for the plugin's dynamic families.
   *
   * The host calls it once with an empty `input` for the candidates that editors always offer,
   * and again with the text of an editor input. A plugin can therefore list a large family only
   * after the input narrows it.
   *
   * @param context - Options of the statement and host operations.
   * @param input - Utility text before the editor's cursor, without variants, such as
   * `icon-mdi--cl`, or an empty string.
   * @returns Utility names in the form that {@link StillwindPlugin.resolve} accepts. The host
   * keeps only names that start with `input`. A name that ends with `-` is incomplete, such as
   * `icon-mdi--`. Editors insert it and request completion again.
   */
  catalog?(context: PluginContext, input: string): readonly string[] | Promise<readonly string[]>;
}

/** Inputs of one loaded `@plugin` statement. */
export interface PluginContext {
  /**
   * Declarations of the statement's options block, keyed by name. Values are the trimmed
   * declaration text without the semicolon. A statement without a block has no options.
   */
  readonly options: Readonly<Record<string, string>>;
  /** Absolute project directory. */
  readonly root: string;
  /**
   * Registers a file that the plugin reads, so that a change reloads the design system.
   *
   * @param path - Absolute file path.
   */
  watch(path: string): void;
  /**
   * Resolves a module request from the stylesheet that contains the statement, with the host's
   * module resolution. A plugin uses it instead of Node APIs, so that it also runs in browser hosts.
   *
   * @param specifier - Package specifier with an optional subpath, such as
   * `@iconify-json/mdi/icons.json`, or a relative or absolute path.
   * @returns The absolute path of the file, or undefined when the request does not resolve.
   */
  resolve(specifier: string): Promise<string | undefined>;
  /**
   * Reads a text file through the host.
   *
   * @param path - Absolute file path, such as a result of {@link PluginContext.resolve}.
   * @returns The file's text.
   * @throws If the file does not exist or cannot be read.
   */
  read(path: string): Promise<string>;
}

/**
 * Returns `plugin` unchanged with its type checked, for use in a configuration file.
 *
 * @param plugin - Plugin definition.
 * @returns The same object.
 */
export function definePlugin(plugin: StillwindPlugin): StillwindPlugin {
  return plugin;
}

/**
 * Parses the options block of a `@plugin` statement.
 *
 * @param text - Trimmed block text without braces, such as `prefix: i; scale: 1.2;`.
 * @returns Declarations keyed by name. A later declaration of a name replaces an earlier one.
 * Text without a colon is ignored. Quotes are kept in values.
 */
export function parsePluginOptions(text: string): Record<string, string> {
  const options: Record<string, string> = {};
  for (const declaration of text.split(";")) {
    const colon = declaration.indexOf(":");
    if (colon < 0) {
      continue;
    }
    const name = declaration.slice(0, colon).trim();
    if (name) {
      options[name] = declaration.slice(colon + 1).trim();
    }
  }
  return options;
}
