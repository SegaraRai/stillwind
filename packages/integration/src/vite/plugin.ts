import type { Plugin } from "vite";

import { buildHooks } from "./build.ts";
import { developmentHooks } from "./development.ts";
import {
  createViteProject,
  type EnvironmentOwner,
  type IntegrationConfig,
  type ViteProject,
} from "./project.ts";
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
  const servers = new Map<ViteProject, Set<EnvironmentOwner>>();
  return [
    {
      name: "stillwind:compiler",
      enforce: "pre",
      /**
       * Initializes the project owned by this configuration and composes Vue source maps.
       * @param config - Resolved Vite configuration whose plugin chain includes Stillwind.
       * @returns Completion of project configuration.
       * @throws If configuration discovery or evaluation fails.
       */
      async configResolved(config) {
        await project.forConfig(config).configure(config);
        attachVueSourceMaps(config.plugins, project);
      },
      /**
       * Defers watch registration until the server's plugin setup succeeds.
       * @param server - Server whose configuration owns the project.
       * @returns A post hook that attaches watches and records the server's environments.
       */
      configureServer(server) {
        const generation = project.forConfig(server.config);
        return () => {
          generation.attachServer(server);
          servers.set(generation, new Set(Object.values(server.environments)));
          server.httpServer?.once("close", generation.closeTypeFacts);
        };
      },
      /**
       * Creates compiler hooks while Vite resolves plugins for an environment.
       * @param environment - Descriptor supplied during plugin resolution. Runtime hooks receive
       * their owning instance as this.environment.
       * @returns Hooks that initialize and close the compiler state owned by this.environment.
       */
      applyToEnvironment(environment) {
        const generation = project.forConfig(environment.getTopLevelConfig());
        return compilerPlugin(generation, (owner) => {
          servers.get(generation)?.delete(owner);
        });
      },
      /** Releases projects after all environments owned by their server have closed. */
      closeServer() {
        for (const [generation, remaining] of servers) {
          if (remaining.size === 0) {
            generation.close();
            servers.delete(generation);
          }
        }
      },
    },
    {
      name: "stillwind:development",
      enforce: "pre",
      apply: "serve",
      configureServer(server) {
        const hook = developmentHooks(project.forConfig(server.config)).configureServer;
        if (typeof hook === "function") {
          return hook.call(this, server);
        }
      },
      applyToEnvironment(environment) {
        const { hotUpdate } = developmentHooks(project.forConfig(environment.getTopLevelConfig()));
        return {
          name: "stillwind:development",
          hotUpdate,
        };
      },
    },
    {
      name: "stillwind:build",
      enforce: "pre",
      apply: "build",
      applyToEnvironment(environment) {
        return {
          name: "stillwind:build",
          ...buildHooks(project.forConfig(environment.getTopLevelConfig())),
        };
      },
    },
  ];
}

/**
 * Creates the plugin that owns project configuration, source compilation, and compiler lifetime.
 *
 * @param project - Project whose environments are initialized at build start and closed at bundle
 * close.
 * @param closed - Records which Vite instance has released its state.
 * @returns A plugin with `enforce: "pre"` that installs virtual modules and source transforms.
 */
function compilerPlugin(
  project: ViteProject,
  closed: (environment: EnvironmentOwner) => void,
): Plugin {
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
     * Refreshes configuration, registers its watches, and starts the current compiler environment.
     * @returns Completion of configuration refresh and environment initialization.
     * @throws If refresh or environment startup fails.
     */
    async buildStart() {
      await project.refreshConfiguration();
      for (const file of project.configurationFiles) {
        this.addWatchFile(file);
      }
      await project.environment(this.environment).start(this);
    },
    /** Releases this environment's state when its plugin container closes. */
    closeBundle() {
      project.closeEnvironment(this.environment);
      closed(this.environment);
    },
  };
}
