import { pathToFileURL } from "node:url";

import type { UpstreamSourceMap } from "@stillwind/host";
import type { LoaderContext } from "webpack";

import type producerLoader from "./loader.ts";
import type { TurbopackLoaderOptions } from "./project.ts";

/** Settings for a compiler loader that executes in Node. */
export type NodeLoaderOptions = TurbopackLoaderOptions & {
  /** Absolute path to the compiled loader implementation. */
  implementation: string;
};

/**
 * Loads the compiler in Node without exposing its dependencies to Turbopack's loader tracing.
 * @param this - Loader context with an absolute implementation path and asynchronous completion.
 * @param source - Original source text forwarded to the implementation.
 * @param inputMap - Upstream source map forwarded unchanged.
 * @returns Undefined. The implementation completes the loader callback and registers its dependencies.
 * Import and synchronous invocation errors complete the callback with an error.
 */
export default function nodeLoader(
  this: LoaderContext<NodeLoaderOptions>,
  source: string,
  inputMap: UpstreamSourceMap,
) {
  const callback = this.async();
  const implementation = pathToFileURL(this.getOptions().implementation).href;
  void import(/* webpackIgnore: true */ implementation)
    .then((module: { default: typeof producerLoader }) =>
      module.default.call(this, source, inputMap),
    )
    .catch(callback);
}
