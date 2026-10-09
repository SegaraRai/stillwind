import type { LoaderContext } from "webpack";

import { emitStylesheet } from "../turbopack/css-loader.ts";
import { routeEntries, type NextLoaderOptions } from "./entries.ts";

/**
 * Reconstructs document CSS after discovering Next routes with evaluated project settings.
 * @param this - Loader context receiving route watches and asynchronous CSS/map completion or an error.
 */
export default function nextCssLoader(this: LoaderContext<NextLoaderOptions>) {
  emitStylesheet(this, (options) =>
    routeEntries(this, {
      ...this.getOptions(),
      root: options.root,
    }),
  );
}
