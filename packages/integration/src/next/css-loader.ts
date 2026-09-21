import type { LoaderContext } from "webpack";

import { emitStylesheet } from "../turbopack/css-loader.ts";
import { routeEntries, type NextLoaderOptions } from "./entries.ts";

export default function (this: LoaderContext<NextLoaderOptions>) {
  emitStylesheet(this, () => routeEntries(this, this.getOptions()));
}
