const { fileURLToPath } = require("node:url");
module.exports = function () {
  const css = new URLSearchParams(this.resourceQuery).get("stillwind");
  if (css === null) throw new Error("Missing Stillwind CSS carrier payload");
  const callback = this.async();
  void import("@stillwind/host")
    .then(({ decodeCssSourceMap }) => {
      const output = decodeCssSourceMap(Buffer.from(css, "base64url").toString("utf8"));
      if (output.map)
        output.map.sources = output.map.sources.map((source) =>
          source.startsWith("file:") ? fileURLToPath(source).replaceAll("\\", "/") : source,
        );
      callback(null, output.code, output.map);
    })
    .catch(callback);
};
