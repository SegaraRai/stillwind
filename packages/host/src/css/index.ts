/**
 * Loads the stylesheet graph that defines a native design system and marks the user's own CSS
 * for stylesheet lint. Plugins of `@plugin` statements generate CSS for the graph and define
 * candidates of their dynamic families on demand. CSS output helpers replace resource placeholders with host delivery URLs
 * and retain the corresponding source maps.
 */

export {
  cssWithSourceMap,
  decodeCssSourceMap,
  type CssLocations,
  type CssSourceMapInput,
  type CssUpstreamMap,
  type DecodedCssSourceMap,
} from "./css-source-map.ts";
export {
  isStylesheet,
  lintsStylesheet,
  loadCssGraph,
  loadEditorCssGraph,
  type CssHost,
  type CssPluginInputs,
  type EditorCssGraph,
  type LoadedCssGraph,
} from "./css.ts";
export { defaultPluginExport, type PluginHost, type TailwindSettings } from "./plugins/host.ts";
export type { PluginPathCompletion } from "./plugins/paths.ts";
export { PluginRuntime } from "./plugins/runtime.ts";
export { attachPlugins, compileWithPlugins, pluginLoader } from "./plugins/session.ts";
