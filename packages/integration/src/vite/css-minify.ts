import { minifyCss } from "@stillwind/native";
import type { Rollup } from "vite";

import type { ProjectSettings } from "./project.ts";

/**
 * Minifies expanded CSS and reports native parser warnings through the bundler.
 * @param css - Generated CSS with valid resource URLs or resource placeholder tokens.
 * @param settings - Browser targets shared with compiler generation.
 * @param context - Bundler warning reporter.
 * @param sourceMap - Optional v3 map for this exact input, composed into the returned map.
 * @returns Minified CSS and an optional separate map without embedded source contents.
 * @throws If native parsing, minification, or source-map composition fails.
 */
export function minifyStylesheet(
  css: string,
  settings: Pick<ProjectSettings, "cssTargets">,
  context: Pick<Rollup.PluginContext, "warn">,
  sourceMap?: string,
) {
  const result = minifyCss(css, {
    filename: "stillwind.css",
    targets: settings.cssTargets,
    sourceMap: sourceMap !== undefined,
    inputSourceMap: sourceMap,
    includeSourcesContent: false,
  });
  for (const warning of result.warnings) {
    context.warn(warning);
  }
  return result;
}
