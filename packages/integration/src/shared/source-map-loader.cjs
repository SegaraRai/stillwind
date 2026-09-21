module.exports = function (source, inputMap, additionalData) {
  const callback = this.async();
  // Rspack 2 stores chain data on loaderItem; webpack exposes it directly.
  // Keep this compatibility boundary local and cover both official runners.
  const transform = this.loaders
    .map((loader) => (loader.data ?? loader.loaderItem?.data)?.stillwindSourceTransform)
    .find((entry) => entry?.resource === this.resourcePath);
  if (!inputMap || !transform) {
    return callback(null, source, inputMap, additionalData);
  }
  void import("@stillwind/host").then((host) => {
    try {
      const map = host.composeFrameworkSourceMap(inputMap, transform.map, transform.code);
      callback(null, source, map, additionalData);
    } catch (error) {
      callback(error);
    }
  }, callback);
};
