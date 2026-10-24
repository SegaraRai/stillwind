import type { SourceMapInput } from "@jridgewell/trace-mapping";
import type { LoaderContext } from "webpack";

import { requireEmission } from "./emission.ts";
import { loaderCompletion, type LoaderAdditionalData } from "./source-map-loader.ts";

/**
 * Skips CSS extraction until the completed graph supplies document CSS. Extracting
 * empty CSS first would retain an empty module in mini-css-extract-plugin's graph.
 * @returns Empty source before reconstruction, otherwise leaves the loader chain active.
 */
export function pitch(this: LoaderContext<unknown>): string | undefined {
  this.cacheable(false);
  if (requireEmission(this).__stillwindCss === undefined) {
    return "";
  }
  return undefined;
}

/** Preserves the host CSS pipeline's output and source map after reconstruction. */
export default function documentLoader(
  this: LoaderContext<unknown>,
  source: string,
  map: SourceMapInput | null | undefined,
  additionalData?: LoaderAdditionalData,
): void {
  loaderCompletion(this)(null, source, map, additionalData);
}
