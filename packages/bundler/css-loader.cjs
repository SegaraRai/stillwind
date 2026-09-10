module.exports = function () {
  const css = new URLSearchParams(this.resourceQuery).get("stillwind");
  if (css === null) throw new Error("Missing Stillwind CSS carrier payload");
  return Buffer.from(css, "base64url").toString("utf8");
};
