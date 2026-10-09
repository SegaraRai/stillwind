import type { Plugin } from "vite";

import { buildHooks } from "./build.ts";
import { developmentHooks } from "./development.ts";
import { createViteProject, type IntegrationConfig, type ViteProject } from "./project.ts";
import { attachVueSourceMaps } from "./source-maps.ts";
import { transformHooks } from "./transform.ts";
import { virtualModuleHooks } from "./virtual-modules.ts";

export type { IntegrationConfig } from "./project.ts";

/**
 * Creates Vite plugins that share compiler state across development and production.
 *
 * @param config - File discovery, project selection, compiler overrides, and CSS emission mode.
 * @returns Plugins for compilation, development CSS delivery, and production emission. Use the
 * array as `plugins: [stillwind()]` to use the project's configuration file.
 */
export function stillwind(config: IntegrationConfig = {}): Plugin[] {
  return createVitePlugins(createViteProject(config));
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
    /**
     * Delegates generated-reference resolution using the current environment's hook context.
     * @param id - Requested module identity.
     * @param importer - Importing resolver identity, when available.
     * @param options - Vite resolution options forwarded unchanged.
     * @returns The virtual resolver's result, or undefined if it has no callable hook.
     */
    resolveId(id, importer, options) {
      if (typeof virtualModules.resolveId === "function") {
        return virtualModules.resolveId.call(this, id, importer, options);
      }
    },
    /**
     * Delegates generated-module loading through the current environment's registry.
     * @param id - Resolved module identity.
     * @param options - Vite load options forwarded unchanged.
     * @returns The virtual loader's result, or undefined when no callable hook is available.
     */
    load(id, options) {
      if (typeof virtualModules.load === "function") {
        return virtualModules.load.call(this, id, options);
      }
    },
    ...transformHooks(project, virtualModules),
    /**
     * Initializes project settings and wraps Vue transforms for source-map composition.
     * @param config - Final Vite configuration including the plugin chain and application root.
     * @returns Completion of project configuration.
     * @throws If configuration discovery or evaluation fails.
     */
    async configResolved(config) {
      await project.configure(config);
      attachVueSourceMaps(config.plugins, project);
    },
    /**
     * Attaches the development server and closes checker resources when its HTTP server closes.
     * @param server - Server retained by the project for resolution, access checks, and HMR.
     */
    configureServer(server) {
      project.attachServer(server);
      server.httpServer?.once("close", project.closeTypeFacts);
    },
    /**
     * Refreshes configuration, registers its watches, and starts the current compiler environment.
     * @returns Completion of configuration refresh and environment initialization.
     * @throws If refresh or environment startup fails.
     */
    async buildStart() {
      await project.refreshConfiguration();
      for (const file of project.configurationFiles) {
        this.addWatchFile(file);
      }
      await project.environment(this.environment?.name).start(this);
    },
    /** Releases all project environments and checker resources when bundle ownership ends. */
    closeBundle() {
      project.close();
    },
  };
}
