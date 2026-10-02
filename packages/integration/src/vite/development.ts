import { resolve } from "node:path";

import { cleanModuleId } from "@stillwind/host";
import type { Plugin, ViteDevServer } from "vite";

import { sendJson } from "./http.ts";
import { valuePrefix } from "./module-ids.ts";
import type { ViteProject } from "./project.ts";

export function developmentHooks(
  project: ViteProject,
): Pick<Plugin, "hotUpdate" | "configureServer"> {
  const {
    settings,
    environmentState,
    closeTypeFacts,
    refreshDesignSystem,
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
          await refreshDesignSystem(state, host);
          await publish(name, host);
          const changed = new Set(state.themeChanges);
          state.themeChanges.clear();
          const modules = [...inputMap(name).values()]
            .filter((input) => input.manual || changed.has(input.moduleId))
            .flatMap((input) => [
              ...(this.environment.moduleGraph.getModulesByFile(cleanModuleId(input.id)) ?? []),
              ...(input.manual
                ? [
                    this.environment.moduleGraph.getModuleById(
                      valuePrefix + Buffer.from(input.id).toString("base64url"),
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
      server.middlewares.use("/__stillwind/style.css", (request, response) => {
        const params = new URL(request.url ?? "", "http://stillwind.local").searchParams;
        const name = params.get("environment") ?? "ssr";
        const selection = params.getAll("module");
        const environment = server.environments[name];
        if (!environment) {
          return sendJson(response, { error: "Unknown environment" }, 404);
        }
        void documentPlan(
          name,
          {
            resolve: (specifier, importer) =>
              environment.pluginContainer.resolveId(specifier, importer),
            addWatchFile: (id) => {
              server.watcher.add(id);
            },
          },
          false,
          selection.length ? selection : undefined,
        )
          .then((plan) => {
            response.setHeader("Content-Type", "text/css; charset=utf-8");
            response.setHeader("Cache-Control", "no-store");
            response.end(plan.css);
          })
          .catch((error) => {
            sendJson(response, { error: String(error) }, 500);
          });
      });
      server.middlewares.use(`${settings.publicBase}__stillwind/css`, (request, response) => {
        const name =
          new URL(request.url ?? "", "http://stillwind.local").searchParams.get("environment") ??
          "client";
        const environment = server.environments[name];
        if (!environment) {
          return sendJson(response, { error: "Unknown environment" }, 404);
        }
        void documentPlan(name, {
          resolve: (specifier: string, importer: string) =>
            environment.pluginContainer.resolveId(specifier, importer),
          addWatchFile: (id: string) => {
            server.watcher.add(id);
          },
        })
          .then((plan) =>
            sendJson(response, {
              revision: plan.revision,
              parts: plan.parts,
              dependencies: Object.fromEntries(
                [...inputMap(name).values()].map((input) => [input.moduleId, input.dependencies]),
              ),
            }),
          )
          .catch((error) => {
            sendJson(response, { error: String(error) }, 500);
          });
      });
      server.httpServer?.once("close", closeTypeFacts);
    },
  };
}
