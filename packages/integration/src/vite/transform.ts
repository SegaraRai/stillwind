import { readFile } from "node:fs/promises";
import { relative, resolve } from "node:path";

import { cleanModuleId } from "@stillwind/host";
import type { Plugin } from "vite";

import { readableFile } from "./file-access.ts";
import type { ViteProject } from "./project.ts";
import { projectionImports } from "./projection-imports.ts";
import { isReactRouterClientRoute } from "./react-router.ts";
import { resolveServerSource } from "./server-source-resolve.ts";

/** Project settings whose configured CSS entrypoint has already produced an import warning. */
const warnedEntrypointImports = new WeakSet<ViteProject["settings"]>();

/** Selects the compiler's position and source after a framework regenerates scripts. */
export interface TransformOptions {
  /** Runs before other pre transforms by default, or in plugin order with `normal`. */
  order?: "pre" | "normal";
  /** Compiles the original file when an earlier framework transform regenerated its script. */
  originalSource?: boolean;
}

/**
 * Creates source transform and watch hooks for the project compiler plugin.
 *
 * Development changes publish CSS after compilation. Invalid projection imports fail through the
 * Vite hook context.
 *
 * @param project - Project configured and initialized by the compiler plugin.
 * @param virtualModules - Virtual module hooks for the same project, used to reload cached
 * generated modules.
 * @param options - Source selection and ordering required by the framework integration.
 * @returns Hooks that recompile cached producers, register stylesheet projections, reserve authored
 * CSS identifiers, and remove deleted inputs. The transform warns once per project when the
 * configured CSS entrypoint is imported as an ordinary stylesheet.
 */
export function transformHooks(
  project: ViteProject,
  virtualModules: Pick<Plugin, "load">,
  options: TransformOptions = {},
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
      order: options.order === "normal" ? undefined : "pre",
      /**
       * Reserves authored CSS names, registers source projections, and compiles supported producers.
       * @param code - Source after earlier transforms in the configured order.
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
          if (
            project.config.css &&
            !warnedEntrypointImports.has(settings) &&
            relative(resolve(settings.projectRoot, project.config.css), cleanModuleId(id)) === ""
          ) {
            warnedEntrypointImports.add(settings);
            this.warn(
              `The Stillwind CSS entrypoint ${project.config.css} is imported as an ordinary stylesheet. ` +
                "The integration already delivers its compiled CSS. Remove the import so that Vite " +
                "does not also emit the uncompiled Stillwind rules.",
            );
          }
          await project.environment(name).reserveAuthoredCss(id, code, this);
          return;
        }
        if (
          !/\.(?:[cm]?[jt]sx?|astro|svelte|vue)$/.test(cleanModuleId(id)) ||
          id.includes("node_modules") ||
          id.startsWith("\0") ||
          /[?&](?:raw|url|type)(?:[=&]|$)/.test(id) ||
          (this.environment && isReactRouterClientRoute(id, this.environment.getTopLevelConfig()))
        ) {
          return;
        }
        if (options.originalSource) {
          id = cleanModuleId(id);
          code = await readFile(await readableFile(id, settings.devServerRef), "utf8");
        }
        const projections = new Map<string, string>();
        for (const specifier of await projectionImports(code, id)) {
          const source = await resolveServerSource(
            this.resolve.bind(this),
            specifier.slice(0, -"?stillwind.css".length),
            id,
          );
          if (!source || source.external) {
            this.error(`Cannot resolve Stillwind projection ${specifier}`);
          }
          await readableFile(source.id, settings.devServerRef);
          projections.set(specifier, source.id);
        }
        if (
          !/stillwind\/(?:alpine|astro|lit|preact|qwik|react|solid|svelte|vanilla|vue|module-css)/.test(
            code,
          )
        ) {
          const input = project.inputs(name).get(id);
          if (input) {
            await project.environment(name).removeSource(id);
            await publish(name, this);
          }
          await project.environment(name).projections(id, projections);
          return;
        }
        await project.environment(name).projections(id, projections);
        const context = options.originalSource
          ? {
              resolve: this.resolve.bind(this),
              addWatchFile: this.addWatchFile.bind(this),
              warn: this.warn.bind(this),
            }
          : this;
        const result = await compile(name, code, id, context);
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
