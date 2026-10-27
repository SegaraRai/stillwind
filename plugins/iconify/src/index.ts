/**
 * Iconify icons as Stillwind utilities.
 *
 * Enable icons with `@plugin "@stillwind/plugin-iconify";` in your stylesheet.
 * Use {@link iconify} in your project's `plugins` list to set options or custom icon sets.
 */

export type { CollectionLoadContext, CollectionSource } from "./collections.ts";
export { svgDirectory, type SvgDirectoryOptions } from "./directory.ts";
export type { IconMode, SvgoConfig } from "./icons.ts";
export { default, iconify, type IconifyOptions, type IconSyntax } from "./plugin.ts";
