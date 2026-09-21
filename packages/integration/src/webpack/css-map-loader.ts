import path from "node:path";
import { fileURLToPath } from "node:url";

// Lightning CSS can leave a carrier reference in its output map when the
// carrier is outside rootContext (the reconstructed source contains ../).
// Compose only that remaining source; maps already composed are untouched.
export default function (this: any, source: string, inputMap: any) {
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
      const result = composeCarrierSourceMap(inputMap, original.map, resource);
      result.sources = result.sources.map((source) =>
        source?.startsWith("file:") ? fileURLToPath(source).replaceAll("\\", "/") : source,
      );
      callback(null, source, result);
    })
    .catch(callback);
}
