import { resolve } from "node:path";

import { cleanModuleId } from "@stillwind/host";
import type { Plugin, ViteDevServer } from "vite";

import { sendJson } from "./http.ts";
import type { ViteProject } from "./project.ts";

/**
 * Returns development hooks that use `project`'s state for each Vite environment. The server hook
 * adds routes for requesting stylesheets. Hot updates compile changed inputs and send CSS before
 * invalidating JS modules. The caller must also install the compiler hooks to initialize project
 * settings.
 *
 * @param project - Shared project configured and attached to the server by the compiler plugin.
 * @returns Hooks for serving stylesheet requests and handling CSS hot updates.
 */
export function developmentHooks(
  project: ViteProject,
): Pick<Plugin, "hotUpdate" | "configureServer"> {
  const { settings, inputs, valueSpecifier, documentPlan, publish, compile } = project;

  return {
    /**
     * Recompiles changed styles and selects JavaScript modules that require hot invalidation.
     *
     * The hook publishes CSS before returning modules for invalidation. Failed updates warn and
     * leave the last published styles in use.
     *
     * @param context - Vite hot-update event with the current server, changed file, and source
     * reader.
     * @returns Modules affected by a design-system update, an empty list for a CSS-only producer
     * change, or `undefined` to retain normal Vite handling.
     */
    async hotUpdate(context) {
      const name = this.environment.name;
      const state = project.environment(name);
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
        if (state.designSystem.cssObservedFiles.has(resolve(context.file))) {
          await state.refreshDesignSystem(host);
          await publish(name, host);
          const changed = state.takeThemeChanges();
          const modules = [...inputs(name).values()]
            .filter((input) => input.manual || changed.has(input.moduleId))
            .flatMap((input) => [
              ...(this.environment.moduleGraph.getModulesByFile(cleanModuleId(input.id)) ?? []),
              ...(input.manual
                ? [
                    this.environment.moduleGraph.getModuleById(
                      valueSpecifier(input.id, name).replace("virtual:stillwind/", "\0stillwind:"),
                    ),
                  ].filter((module): module is NonNullable<typeof module> => Boolean(module))
                : []),
            ]);
          for (const module of modules) {
            this.environment.moduleGraph.invalidateModule(module);
          }
          return modules;
        }
        const previous = inputs(name).get(id);
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
    /**
     * Registers routes that serve document CSS and revisioned stylesheet snapshots.
     *
     * The document route supports repeated `module` selections and defaults to `ssr`. The snapshot
     * route defaults to `client`. Unknown environments return 404 and stylesheet failures return
     * 500.
     *
     * @param server - Active server with the project's initialized Vite environments.
     */
    configureServer(server: ViteDevServer) {
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
        void project
          .snapshot(name, {
            resolve: (specifier, importer) =>
              environment.pluginContainer.resolveId(specifier, importer),
            addWatchFile: (id) => {
              server.watcher.add(id);
            },
          })
          .then((snapshot) => sendJson(response, snapshot))
          .catch((error) => {
            sendJson(response, { error: String(error) }, 500);
          });
      });
    },
  };
}
