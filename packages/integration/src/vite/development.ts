import { resolve } from "node:path";

import { cleanModuleId } from "@stillwind/host";
import type { Plugin, ViteDevServer } from "vite";

import type { CssContext } from "./design-system.ts";
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
     * @returns Modules affected by a design-system update, the changed modules together with the
     * producers whose JavaScript changed after an edit of a file that they read as a dependency, an
     * empty list when a producer edit changes only CSS and no request has compiled the edited source
     * yet, or `undefined` to retain normal Vite handling.
     */
    async hotUpdate(context) {
      const name = this.environment.name;
      const state = project.environment(name);
      const id = cleanModuleId(context.file);
      const host: CssContext = {
        resolve: (specifier, importer, options) =>
          this.environment.pluginContainer.resolveId(specifier, importer, options),
        addWatchFile: (id: string) => {
          context.server.watcher.add(id);
        },
        warn: (message: string) => {
          this.warn(message);
        },
      };
      try {
        if (state.designSystem.observes(resolve(context.file), context.type !== "update")) {
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
            this.environment.moduleGraph.invalidateModule(
              module,
              new Set(),
              context.timestamp,
              true,
            );
          }
          return modules;
        }
        const previous = inputs(name).get(id);
        let consumers = [...inputs(name).values()].filter(
          (input) => !input.manual && input.id !== id && input.sourceDependencies.has(id),
        );
        const source = previous && context.type !== "delete" ? await context.read() : undefined;
        const result = source === undefined ? undefined : await compile(name, source, id, host);
        // Producers read only the exports of the changed file. When the exports are unchanged,
        // compiling the producers again gives the same JavaScript and CSS. Their checker
        // diagnostics are then not refreshed until they are compiled for another reason.
        if (result !== undefined && consumers.length > 0 && state.keepsInterface(id)) {
          consumers = [];
        }
        // Producers that read the changed file are compiled again here instead of through Vite
        // watch files. Only producers whose JavaScript changes need a hot update, because class
        // names do not depend on the text of the modules that declare the composed styles.
        const stale = [];
        for (const consumer of consumers) {
          let code: string | undefined;
          if (context.type === "update") {
            try {
              code = (await compile(name, consumer.code, consumer.id, host))?.code;
            } catch {
              // The transform that the hot update starts reports the failure.
            }
          }
          const module =
            code === consumer.javascript
              ? undefined
              : this.environment.moduleGraph.getModuleById(consumer.id);
          if (module) {
            this.environment.moduleGraph.invalidateModule(
              module,
              new Set(),
              context.timestamp,
              true,
            );
            stale.push(module);
          }
        }
        if (source !== undefined || consumers.length > 0) {
          try {
            await publish(name, host);
          } catch (error) {
            if (!stale.length) {
              throw error;
            }
            // A producer that failed to compile blocks the stylesheet until the transform that
            // this update starts reports the failure.
            this.warn(`Stillwind update failed; retaining published styles: ${String(error)}`);
            return [...new Set([...context.modules, ...stale])];
          }
        }
        if (previous && source !== undefined) {
          const values = [];
          for (const input of inputs(name).values()) {
            if (!input.manual) {
              continue;
            }
            const value = this.environment.moduleGraph.getModuleById(
              valueSpecifier(input.id, name).replace("virtual:stillwind/", "\0stillwind:"),
            );
            if (!value) {
              continue;
            }
            const plan = await state.modulePlan([input.moduleId], host, false);
            if (plan.producers.some((producer) => producer.module === previous.moduleId)) {
              // Framework hooks can filter this module out of the update list. The HMR
              // timestamp still makes its importers request the new CSS value.
              this.environment.moduleGraph.invalidateModule(
                value,
                new Set(),
                context.timestamp,
                true,
              );
              values.push(value);
            }
          }
          if (values.length || stale.length) {
            return [...new Set([...context.modules, ...values, ...stale])];
          }
          // A transform request can compile the changed source before this hook runs, for example
          // when TanStack Start transforms a route for the server. The recorded output then
          // matches the new output, but the module runner still holds the module from before the
          // change.
          if (
            !previous.manual &&
            previous.code !== source &&
            result?.code === previous.javascript
          ) {
            return [];
          }
        }
        if (stale.length) {
          return [...new Set([...context.modules, ...stale])];
        }
      } catch (error) {
        this.warn(`Stillwind update failed; retaining published styles: ${String(error)}`);
      }
      return undefined;
    },
    /**
     * Registers routes that serve document CSS and revisioned stylesheet snapshots.
     *
     * The document route supports repeated `module` selections and defaults to `ssr`. It records each
     * served URL so that later CSS changes in that environment reload the document's link. The
     * snapshot route defaults to `client`. Unknown environments return 404 and stylesheet failures return
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
        const link = new URL(request.originalUrl ?? request.url ?? "", "http://stillwind.local");
        link.searchParams.delete("t");
        project.observeDocumentStylesheet(name, link.pathname + link.search);
        void documentPlan(
          name,
          {
            resolve: (specifier, importer, options) =>
              environment.pluginContainer.resolveId(specifier, importer, options),
            addWatchFile: (id) => {
              server.watcher.add(id);
            },
            warn: (message) => {
              server.config.logger.warn(message);
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
            resolve: (specifier, importer, options) =>
              environment.pluginContainer.resolveId(specifier, importer, options),
            addWatchFile: (id) => {
              server.watcher.add(id);
            },
            warn: (message) => {
              server.config.logger.warn(message);
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
