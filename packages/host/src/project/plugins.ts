import { createRequire } from "node:module";
import { pathToFileURL } from "node:url";

import type { StillwindPlugin } from "@stillwind/config";

/**
 * Loads the plugin package that an `@plugin` statement names, with Node module resolution from
 * the stylesheet that contains the statement.
 *
 * @param specifier - Package specifier or path from the statement.
 * @param importer - Absolute path of the stylesheet.
 * @returns The plugin that the module exports by default.
 * @throws If resolution or evaluation fails, or the default export is not a plugin.
 */
export async function loadNodePlugin(
  specifier: string,
  importer: string,
): Promise<StillwindPlugin> {
  const resolved = createRequire(importer).resolve(specifier);
  const module = (await import(pathToFileURL(resolved).href)) as { default?: unknown };
  const plugin = module.default;
  if (
    typeof plugin !== "object" ||
    plugin === null ||
    typeof (plugin as StillwindPlugin).name !== "string" ||
    typeof (plugin as StillwindPlugin).css !== "function"
  ) {
    throw new Error(`Stillwind plugin ${specifier} must export a plugin object by default`);
  }
  return plugin as StillwindPlugin;
}
