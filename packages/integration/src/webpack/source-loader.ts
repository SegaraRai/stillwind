import type { UpstreamSourceMap } from "@stillwind/host";
import type { LoaderContext } from "webpack";

/**
 * Returns source text as a module value so importModule never executes application code.
 * @param this - Loader context retained for compatibility with the bundler loader protocol.
 * @param source - Source produced by preceding loaders, serialized as a string literal.
 * @param map - Optional upstream map serialized as a module export. Omission exports null.
 * @returns JavaScript exporting captured source and its map without evaluating that source.
 */
export default function webpackSourceLoader(
  this: LoaderContext<unknown>,
  source: string,
  map: UpstreamSourceMap,
) {
  return `// Stillwind source loader (webpack)
export const source = ${JSON.stringify(String(source))};
export const map = ${JSON.stringify(map ?? null)};
`;
}
