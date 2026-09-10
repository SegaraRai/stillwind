module.exports = function () {
  return new URLSearchParams(this.resourceQuery.slice(1)).get("sw") || "";
};
