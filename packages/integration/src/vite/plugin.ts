import { resolve } from "node:path";

import { cleanModuleId, compilerModuleId } from "@stillwind/host";
import type { Plugin } from "vite";

import { buildHooks } from "./build.ts";
import { developmentHooks } from "./development.ts";
import { createViteProject, type StillwindOptions, type ViteProject } from "./project.ts";
import { attachVueSourceMaps } from "./source-maps.ts";
import { virtualModuleHooks } from "./virtual-modules.ts";

export type { StillwindOptions } from "./project.ts";

/**
 * Compiles producers and links their styles for each Vite environment.
 */
export function stillwind(options: StillwindOptions): Plugin {
  return compilerPlugin(createViteProject(options));
}

/**
 * Returns compiler hooks for `project`, which is also used by the framework adapter.
 * The hooks set project settings from Vite configuration and keep a compiler session
 * for each environment. Server shutdown removes watch listeners and closes any
 * TypeScriptFactsService instances.
 *
 * @internal
 */
export function compilerPlugin(project: ViteProject): Plugin {
  const { options } = project;
  const {
    settings,
    environmentState,
    closeTypeFacts,
    close,
    ensureSession,
    startSession,
    inputMap,
    publish,
    compile,
  } = project;
  return {
    name: "stillwind",
    enforce: "pre",
    perEnvironmentStartEndDuringDev: true,
    perEnvironmentWatchChangeDuringDev: true,
    ...virtualModuleHooks(project),
    ...buildHooks(project),
    ...developmentHooks(project),
    configResolved(config) {
      settings.projectRoot = resolve(config.root, options.root ?? ".");
      settings.assetsDirectory = config.build.assetsDir;
      settings.publicBase = config.base;
      settings.sourceMaps = Boolean(config.build.sourcemap);
      settings.devSourceMaps = Boolean(config.css.devSourcemap);
      settings.minifyCss = config.build.cssMinify !== false;
      settings.watching = config.command === "serve";
      settings.compilerMode = settings.watching ? "development" : "release";
      attachVueSourceMaps(config.plugins, project);
      closeTypeFacts();
    },
    async buildStart() {
      const name = this.environment?.name;
      await startSession(name, this);
    },
    shouldTransformCachedModule({ id }) {
      return (
        /\.(?:[cm]?[jt]sx?|astro|svelte|vue)$/.test(cleanModuleId(id)) &&
        !id.includes("node_modules")
      );
    },
    async watchChange(id, change) {
      if (change.event !== "delete") {
        return;
      }
      const name = this.environment?.name;
      const state = environmentState(name);
      if (!state.session) {
        return;
      }
      let removed = false;
      for (const [key, input] of inputMap(name)) {
        if (cleanModuleId(input.id) === cleanModuleId(id)) {
          inputMap(name).delete(key);
          state.session.remove(input.moduleId);
          removed = true;
        }
      }
      if (removed) {
        await publish(name, this);
      }
    },
    transform: {
      order: "pre",
      async handler(code, id) {
        const name = this.environment?.name;
        if (isAuthoredStylesheet(id)) {
          const state = environmentState(name);
          await ensureSession(state, this);
          state.session!.reserveAuthoredCss(
            compilerModuleId(settings.projectRoot, cleanModuleId(id)),
            code,
          );
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
        if (!/stillwind\/(?:react|preact|solid|astro|svelte|vue|module-css)/.test(code)) {
          const input = inputMap(name).get(id);
          if (input) {
            inputMap(name).delete(id);
            environmentState(name).session?.remove(input.moduleId);
            await publish(name, this);
          }
          return;
        }
        const result = await compile(name, code, id, this);
        await publish(name, this);
        return result;
      },
    },
    closeBundle: close,
  };
}

/**
 * Whether one module id is a stylesheet the host authored, rather than one Stillwind generated.
 *
 * The design system's own sources are reserved already, so reserving them again changes nothing;
 * the carriers the plugin serves are generated CSS and must never be read as authored names.
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
