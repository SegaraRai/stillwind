/**
 * Iconify icons as Stillwind utilities.
 *
 * `@plugin "@stillwind/plugin-iconify";` in a stylesheet enables the utilities. {@link iconify}
 * creates an instance with other defaults or configured icon sets for the `plugins` configuration entry.
 */

export type { CollectionSource } from "./collections.ts";
export type { IconMode } from "./icons.ts";
export { default, iconify, type IconifyOptions, type IconSyntax } from "./plugin.ts";
