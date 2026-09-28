import { resolve } from "node:path";

import { cleanModuleId, compilerModuleId } from "@stillwind/host";
import type { Plugin } from "vite";

import { buildHooks } from "./build.ts";
import { developmentHooks } from "./development.ts";
import { createViteProject, type StillwindOptions } from "./project.ts";
import { virtualModuleHooks } from "./virtual-modules.ts";

export type { StillwindOptions } from "./project.ts";

/** Compiles producers and links their styles for each Vite environment. */
export function stillwind(options: StillwindOptions): Plugin {
  const project = createViteProject(options);
  const { settings, environmentState, closeTypeFacts, ensureSession, inputMap, publish, compile } =
    project;
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
      closeTypeFacts();
    },
    async buildStart() {
      const name = this.environment?.name;
      const state = environmentState(name);
      state.session = undefined;
      inputMap(name).clear();
      state.buildCss = undefined;
      state.manifest = undefined;
      await ensureSession(state, this);
    },
    shouldTransformCachedModule({ id }) {
      return (
        /\.(?:[cm]?[jt]sx?|vue|svelte|astro)$/.test(cleanModuleId(id)) &&
        !id.includes("node_modules")
      );
    },
    async watchChange(id, change) {
      if (change.event !== "delete") {
        return;
      }
      const name = this.environment?.name;
      const state = environmentState(name);
      if (!state.session || !inputMap(name).delete(cleanModuleId(id))) {
        return;
      }
      state.session.remove(compilerModuleId(settings.projectRoot, cleanModuleId(id)));
      await publish(name, this);
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
          !/\.(?:[cm]?[jt]sx?|vue|svelte|astro)$/.test(cleanModuleId(id)) ||
          id.includes("node_modules") ||
          id.startsWith("\0") ||
          /[?&](?:raw|url|type)(?:[=&]|$)/.test(id)
        ) {
          return;
        }
        if (!/stillwind\/(?:react|preact|solid|vue|svelte|astro|module-css)/.test(code)) {
          if (inputMap(name).delete(cleanModuleId(id))) {
            environmentState(name).session?.remove(
              compilerModuleId(settings.projectRoot, cleanModuleId(id)),
            );
            await publish(name, this);
          }
          return;
        }
        const result = await compile(name, code, id, this);
        await publish(name, this);
        return result;
      },
    },
    closeBundle: closeTypeFacts,
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
