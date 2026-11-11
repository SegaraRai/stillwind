import type { ResolvedConfig } from "vite";

/**
 * Identifies React Router's child compiler that transforms routes only to read export names.
 *
 * @param config - Resolved configuration of the compiler that owns the hook.
 * @returns True for the child compiler with React Router's route plugins and without its main
 * plugin, in both development and builds. The child does not generate application JavaScript or
 * CSS.
 */
export function isReactRouterExportScan(
  config: Pick<ResolvedConfig, "command" | "plugins">,
): boolean {
  return (
    config.command === "serve" &&
    config.plugins.some((plugin) => plugin.name === "react-router:virtual-modules") &&
    !config.plugins.some((plugin) => plugin.name === "react-router")
  );
}

/**
 * Identifies a client route module that React Router replaces with re-exports of its route file.
 *
 * @param id - Full Vite identity of the module.
 * @param config - Resolved configuration of the environment that transforms the module.
 * @returns True when React Router's build plugin selects the module through its query. Its later
 * transform discards the code of the earlier transforms, and the route file supplies the styles.
 */
export function isReactRouterClientRoute(
  id: string,
  config: Pick<ResolvedConfig, "plugins">,
): boolean {
  return (
    id.endsWith("?__react-router-build-client-route") &&
    config.plugins.some((plugin) => plugin.name === "react-router:build-client-route")
  );
}
