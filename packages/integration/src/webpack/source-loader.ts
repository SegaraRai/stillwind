import type { UpstreamSourceMap } from "@stillwind/host";
import type { LoaderContext } from "webpack";

/**
 * Returns source text as a module value so importModule never executes application code.
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
