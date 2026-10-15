import { cleanModuleId } from "@stillwind/host";
import type { Plugin } from "vite";

import { readableFile } from "./file-access.ts";
import type { ViteProject } from "./project.ts";
import { projectionImports } from "./projection-imports.ts";

/**
 * Creates source transform and watch hooks for the project compiler plugin.
 *
 * Development changes publish CSS after compilation. Invalid projection imports fail through the
 * Vite hook context.
 *
 * @param project - Project configured and initialized by the compiler plugin.
 * @param virtualModules - Virtual module hooks for the same project, used to reload cached
 * generated modules.
 * @returns Hooks that recompile cached producers, register stylesheet projections, reserve authored
 * CSS identifiers, and remove deleted inputs.
 */
export function transformHooks(
  project: ViteProject,
  virtualModules: Pick<Plugin, "load">,
): Pick<Plugin, "shouldTransformCachedModule" | "watchChange" | "transform"> {
  const { settings, publish, compile } = project;
  const cachedVirtualModules = new Map<string, Set<string>>();

  return {
    /**
     * Selects cached modules that need current compiler state and records virtual modules for reloading.
     * @param module - Cached module descriptor whose id includes any source query.
     * @returns Whether to rerun transformation for Stillwind virtual modules or supported application sources.
     */
    shouldTransformCachedModule({ id }) {
      if (id.startsWith("\0stillwind:")) {
        const name = this.environment?.name ?? "client";
        let modules = cachedVirtualModules.get(name);
        if (!modules) {
          modules = new Set();
          cachedVirtualModules.set(name, modules);
        }
        modules.add(id);
        return true;
      }
      return (
        /\.(?:[cm]?[jt]sx?|astro|svelte|vue)$/.test(cleanModuleId(id)) &&
        !id.includes("node_modules")
      );
    },
    /**
     * Invalidates changed configuration or removes deleted source state and publishes resulting CSS.
     * @param id - Watched filesystem identity.
     * @param change - Vite event. Source removal is performed only for delete events.
     * @returns Completion of removal and publication when required.
     * @throws If environment removal or CSS publication fails.
     */
    async watchChange(id, change) {
      if (project.configurationChanged(id)) {
        return;
      }
      if (change.event !== "delete") {
        return;
      }
      const name = this.environment?.name;
      const removed = await project.environment(name).removeFile(id);
      if (removed) {
        await publish(name, this);
      }
    },
    transform: {
      order: "pre",
      /**
       * Reserves authored CSS names, registers source projections, and compiles supported producers.
       * @param code - Source before ordinary framework lowering.
       * @param id - Full Vite identity, including queries that select distinct source.
       * @returns Compiler output, a reloaded virtual module, or undefined when no transformed source is needed.
       * @throws If a projection cannot resolve or pass access checks, or compilation or publication fails.
       */
      async handler(code, id) {
        const name = this.environment?.name;
        if (cachedVirtualModules.get(name ?? "client")?.delete(id)) {
          const load = virtualModules.load;
          if (typeof load === "function") {
            return load.call(this, id);
          }
        }
        if (isAuthoredStylesheet(id)) {
          await project.environment(name).reserveAuthoredCss(id, code, this);
          return;
        }
        if (
          !/\.(?:[cm]?[jt]sx?|astro|svelte|vue)$/.test(cleanModuleId(id)) ||
          id.includes("node_modules") ||
          id.startsWith("\0") ||
          /[?&](?:raw|url|type)(?:[=&]|$)/.test(id)
        ) {
          return;
        }
        const projections = new Map<string, string>();
        for (const specifier of await projectionImports(code, id)) {
          const source = await this.resolve(specifier.slice(0, -"?stillwind.css".length), id, {
            skipSelf: true,
            custom: {
              "react-router:dot-server": true,
            },
          });
          if (!source || source.external) {
            this.error(`Cannot resolve Stillwind projection ${specifier}`);
          }
          await readableFile(source.id, settings.devServerRef);
          projections.set(specifier, source.id);
        }
        if (!/stillwind\/(?:react|preact|solid|astro|svelte|vue|module-css)/.test(code)) {
          const input = project.inputs(name).get(id);
          if (input) {
            await project.environment(name).removeSource(id);
            await publish(name, this);
          }
          await project.environment(name).projections(id, projections);
          return;
        }
        await project.environment(name).projections(id, projections);
        const result = await compile(name, code, id, this);
        await publish(name, this);
        return result;
      },
    },
  };
}

/**
 * Checks whether a module contains authored CSS whose identifiers must be reserved.
 *
 * @param id - Vite module identity, optionally containing a resource query.
 * @returns True for a plain `.css` file that is neither virtual nor query-qualified.
 */
function isAuthoredStylesheet(id: string): boolean {
  if (!cleanModuleId(id).endsWith(".css")) {
    return false;
  }
  if (id.startsWith("virtual:") || id.includes("?")) {
    return false;
  }
  const path = cleanModuleId(id).replaceAll("\\", "/");
  return !path.startsWith("\0stillwind:");
}
