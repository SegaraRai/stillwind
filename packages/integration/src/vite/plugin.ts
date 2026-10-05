import type { Plugin } from "vite";

import { buildHooks } from "./build.ts";
import { developmentHooks } from "./development.ts";
import { createViteProject, type StillwindOptions, type ViteProject } from "./project.ts";
import { attachVueSourceMaps } from "./source-maps.ts";
import { transformHooks } from "./transform.ts";
import { virtualModuleHooks } from "./virtual-modules.ts";

export type { StillwindOptions } from "./project.ts";

/**
 * Creates Vite plugins that share compiler state across development and production.
 *
 * @param options - Compiler options and CSS emission mode for this integration instance.
 * @returns Plugins for compilation, development CSS delivery, and production emission. Use the
 * array as `plugins: [stillwind(options)]`.
 */
export function stillwind(options: StillwindOptions): Plugin[] {
  return createVitePlugins(createViteProject(options));
}

/**
 * Creates the compiler, development, and build plugins around an existing project.
 *
 * @param project - Project shared by all returned plugins. Its settings are initialized by the
 * compiler plugin.
 * @returns Plugins in compiler, development, and build order. Development and build plugins apply
 * only to their matching Vite command.
 */
export function createVitePlugins(project: ViteProject): Plugin[] {
  return [
    compilerPlugin(project),
    {
      name: "stillwind:development",
      enforce: "pre",
      apply: "serve",
      ...developmentHooks(project),
    },
    {
      name: "stillwind:build",
      enforce: "pre",
      apply: "build",
      ...buildHooks(project),
    },
  ];
}

/**
 * Creates the plugin that owns project configuration, source compilation, and compiler lifetime.
 *
 * @param project - Project whose environments are initialized at build start and closed at bundle
 * close.
 * @returns A plugin with `enforce: "pre"` that installs virtual modules, source transforms, and
 * Vue source-map composition.
 */
function compilerPlugin(project: ViteProject): Plugin {
  const virtualModules = virtualModuleHooks(project);

  return {
    name: "stillwind:compiler",
    enforce: "pre",
    perEnvironmentStartEndDuringDev: true,
    perEnvironmentWatchChangeDuringDev: true,
    ...virtualModules,
    ...transformHooks(project, virtualModules),
    configResolved(config) {
      project.configure(config);
      attachVueSourceMaps(config.plugins, project);
    },
    configureServer(server) {
      project.attachServer(server);
      server.httpServer?.once("close", project.closeTypeFacts);
    },
    async buildStart() {
      await project.environment(this.environment?.name).start(this);
    },
    closeBundle() {
      project.close();
    },
  };
}
