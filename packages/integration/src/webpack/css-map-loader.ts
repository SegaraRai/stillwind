import path from "node:path";
import { fileURLToPath } from "node:url";

/**
 * Composes remaining CSS carrier references in the host pipeline's output map.
 *
 * Lightning CSS can retain a carrier source outside `rootContext`. Only sources resolving to this
 * loader's absolute resource path are composed. Already composed sources stay outside this step.
 * The original CSS and its inline map are decoded from the `stillwind` query payload.
 *
 * A payload used for composition must contain CSS with a trailing inline map recognized by
 * `decodeCssSourceMap`.
 *
 * @param this - Loader context with the resource path, query, dependency registration, and
 * synchronous and asynchronous completion callbacks.
 * @param source - CSS text produced by the preceding loaders. This loader returns it unchanged.
 * @param inputMap - Output map from the host CSS pipeline. Without a map or payload, it is passed
 * through unchanged.
 * @returns The loader callback's result for passthrough, or undefined while asynchronous
 * composition completes. Decoding and composition failures are delivered to the callback.
 */
export default function webpackCssMapLoader(this: any, source: string, inputMap: any) {
  this.addDependency(import.meta.filename);
  const payload = new URLSearchParams(this.resourceQuery).get("stillwind");
  if (!payload || !inputMap) {
    return this.callback(null, source, inputMap);
  }
  const callback = this.async();
  const resource = path.resolve(this.resourcePath).replaceAll("\\", "/");
  void import("@stillwind/host")
    .then(({ decodeCssSourceMap, composeCarrierSourceMap }) => {
      const original = decodeCssSourceMap(Buffer.from(payload, "base64url").toString("utf8"));
      const result = composeCarrierSourceMap(inputMap, original.map!, resource);
      result.sources = result.sources.map((source) =>
        source?.startsWith("file:") ? fileURLToPath(source).replaceAll("\\", "/") : source,
      );
      callback(null, source, result);
    })
    .catch(callback);
}
