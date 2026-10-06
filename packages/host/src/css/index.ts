/**
 * Loads the stylesheet graph that defines a native design system. CSS output helpers replace
 * resource placeholders with host delivery URLs and retain the corresponding source maps.
 */
export {
  cssWithSourceMap,
  decodeCssSourceMap,
  type CssLocations,
  type CssSourceMapInput,
  type CssUpstreamMap,
  type DecodedCssSourceMap,
} from "./css-source-map.ts";
export { loadCssGraph, type CssHost, type LoadedCssGraph } from "./css.ts";
