export default function (this: any, source: string, inputMap: any, additionalData: any) {
  const callback = this.async();

  // Vue development script maps include the template. Replacing their embedded source
  // changes script hashes on template edits and makes Vue remount the component.
  if (
    this.mode !== "production" &&
    this._module?.matchResource &&
    /[?&]vue&type=script(?:&|$)/.test(this.resourceQuery ?? "")
  ) {
    return callback(null, source, inputMap, additionalData);
  }

  // Rspack 2 stores chain data on loaderItem; webpack exposes it directly.
  // Keep this compatibility boundary local and cover both official runners.
  const transform = this.loaders
    .map((loader: any) => (loader.data ?? loader.loaderItem?.data)?.stillwindSourceTransform)
    .find((entry: any) => entry?.resource === this.resourcePath);
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
}
