import { fileURLToPath } from "node:url";

import { registerEmission } from "./emission.ts";

export default function (this: any) {
  this.addDependency(import.meta.filename);
  const document = new URLSearchParams(this.resourceQuery).has("stillwind-document");
  if (!document) {
    throw new Error("Unknown Stillwind stylesheet request");
  }
  this.cacheable(false);
  registerEmission(this);
  const css = this._compilation?.__stillwindCss ?? "";
  const callback = this.async();
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
