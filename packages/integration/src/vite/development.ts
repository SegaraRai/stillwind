import { resolve } from "node:path";

import { cleanModuleId } from "@stillwind/host";
import type { Plugin, ViteDevServer } from "vite";

import { writeVocabulary } from "../options.ts";
import { sendJson } from "./http.ts";
import { valuePrefix } from "./module-ids.ts";
import type { ViteProject } from "./project.ts";
import { resolvedFile } from "./resources.ts";

export function developmentHooks(
  project: ViteProject,
): Pick<Plugin, "hotUpdate" | "configureServer"> {
  const {
    options,
    settings,
    environmentState,
    closeTypeFacts,
    loadDesignSystem,
    inputMap,
    documentPlan,
    publish,
    compile,
  } = project;
  return {
    async hotUpdate(context) {
      const name = this.environment.name;
      const state = environmentState(name);
      const id = cleanModuleId(context.file);
      const host = {
        resolve: (specifier: string, importer: string) =>
          this.environment.pluginContainer.resolveId(specifier, importer),
        addWatchFile: (id: string) => {
          context.server.watcher.add(id);
        },
        warn: (message: string) => {
          this.warn(message);
        },
      };
      try {
        if (state.cssFiles.has(context.file) || state.cssFiles.has(resolve(context.file))) {
          const revision = ++state.loadRevision;
          const dependencies = new Set<string>();
          const { loader, resources } = await loadDesignSystem(
            async (specifier, importer) =>
              resolvedFile(await host.resolve(specifier, importer), `CSS ${specifier}`),
            dependencies,
            state.session,
          );
          if (revision !== state.loadRevision) {
            return [];
          }
          const update = state.session!.reload(loader);
          state.resources = resources;
          state.cssFiles = dependencies;
          context.server.watcher.add([...dependencies]);
          writeVocabulary(loader, settings.projectRoot, options);
          await publish(name, host);
          const changed = new Set(update.transformedModules);
          const modules = [...inputMap(name).values()]
            .filter((input) => input.manual || changed.has(input.moduleId))
            .flatMap((input) => [
              ...(this.environment.moduleGraph.getModulesByFile(cleanModuleId(input.id)) ?? []),
              ...(input.manual
                ? [
                    this.environment.moduleGraph.getModuleById(
                      valuePrefix + Buffer.from(cleanModuleId(input.id)).toString("base64url"),
                    ),
                  ].filter((module): module is NonNullable<typeof module> => Boolean(module))
                : []),
            ]);
          for (const module of modules) {
            this.environment.moduleGraph.invalidateModule(module);
          }
          return modules;
        }
        const previous = inputMap(name).get(id);
        if (previous && context.type !== "delete") {
          const result = await compile(name, await context.read(), id, host);
          await publish(name, host);
          if (!previous.manual && result?.code === previous.javascript) {
            return [];
          }
        }
      } catch (error) {
        this.warn(`Stillwind update failed; retaining published styles: ${String(error)}`);
      }
      return undefined;
    },
    configureServer(server: ViteDevServer) {
      settings.devServerRef = server;
      server.middlewares.use("/__stillwind/css", (request, response) => {
        const name =
          new URL(request.url ?? "", "http://stillwind.local").searchParams.get("environment") ??
          "client";
        const environment = server.environments[name];
        if (!environment) {
          return sendJson(response, { error: "Unknown environment" }, 404);
        }
        const host = {
          resolve: (specifier: string, importer: string) =>
            environment.pluginContainer.resolveId(specifier, importer),
          addWatchFile: (id: string) => {
            server.watcher.add(id);
          },
        };
        void documentPlan(name, host)
          .then((plan) =>
            sendJson(response, {
              revision: plan.revision,
              parts: plan.parts,
              dependencies: Object.fromEntries(
                [...inputMap(name).values()].map((input) => [input.moduleId, input.dependencies]),
              ),
            }),
          )
          .catch((error) => sendJson(response, { error: String(error) }, 500));
      });
      server.httpServer?.once("close", closeTypeFacts);
    },
  };
}
