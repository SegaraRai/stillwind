import { fileURLToPath } from "node:url";

import type { LoaderContext } from "webpack";

import { requireEmission } from "./emission.ts";
import { loaderCompletion } from "./source-map-loader.ts";

/**
 * Supplies reconstructed document CSS to the host CSS pipeline through the asynchronous loader
 * callback.
 *
 * The loader context must identify a `stillwind-document` request in a compilation registered by
 * `StillwindPlugin`. The loader disables caching because the carrier is rebuilt after graph
 * linking. The first pass supplies empty CSS until reconstruction has stored the completed
 * stylesheet.
 *
 * @param this - Document carrier context with registered emission state and asynchronous completion.
 * @throws If the request is not a document carrier or the compilation has no registered emission
 * hook.
 */
export default function webpackCssLoader(this: LoaderContext<unknown>) {
  this.addDependency(import.meta.filename);
  const document = new URLSearchParams(this.resourceQuery).has("stillwind-document");
  if (!document) {
    throw new Error("Unknown Stillwind stylesheet request");
  }
  this.cacheable(false);
  const css = requireEmission(this).__stillwindCss ?? "";
  const callback = loaderCompletion(this);
  void import("@stillwind/host")
    .then(({ decodeCssSourceMap }) => {
      const output = decodeCssSourceMap(css);
      if (output.map) {
        output.map.sources = output.map.sources.map((source: string) =>
          source.startsWith("file:") ? fileURLToPath(source).replaceAll("\\", "/") : source,
        );
      }
      callback(null, output.code, output.map);
    })
    .catch(callback);
}
