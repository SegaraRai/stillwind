import type { LoaderContext } from "webpack";

/** Returns source text as a module value so importModule never executes application code. */
export default function (this: LoaderContext<unknown>, source: string, map: unknown) {
  return `export const source=${JSON.stringify(String(source))};export const map=${JSON.stringify(map ?? null)};`;
}
