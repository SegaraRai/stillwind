import type { LoaderContext } from "webpack";

import { emitStylesheet } from "../turbopack/css-loader.ts";
import { routeEntries, type NextLoaderOptions } from "./entries.ts";

export default function nextCssLoader(this: LoaderContext<NextLoaderOptions>) {
  emitStylesheet(this, (options) =>
    routeEntries(this, {
      ...this.getOptions(),
      root: options.root,
    }),
  );
}
