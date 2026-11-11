import { readFile } from "node:fs/promises";
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
): Pick<Plugin, "hotUpdate" | "configureServer" | "watchChange"> {
  const { settings, inputs, valueSpecifier, documentPlan, publish, compile } = project;

  /**
   * Compilations of changed files that {@link Plugin.watchChange} started for the next hot update,
   * keyed by file path without a query.
   */
  const pending = new Map<string, Promise<CompiledChange>>();

  /**
   * Compiles a changed producer and the producers that read the changed file as a dependency.
   *
   * @param name - Environment name.
   * @param id - Changed file path without a query.
   * @param type - Vite file event.
   * @param read - Reads the current text of the changed file.
   * @param host - Resolution, watch, and warning operations of the calling hook.
   * @returns The recorded input before the change, the new source and its compilation when the
   * file is a recorded producer, the consumers to check, and their new JavaScript. A consumer
   * whose compilation fails or that is not compiled for the event has no JavaScript.
   * @throws If the compilation of the changed producer fails.
   */
  const compileChange = async (
    name: string,
    id: string,
    type: "create" | "update" | "delete",
    read: () => string | Promise<string>,
    host: CssContext,
  ) => {
    const state = project.environment(name);
    const previous = inputs(name).get(id);
    let consumers = [...inputs(name).values()].filter(
      (input) => !input.manual && input.id !== id && input.sourceDependencies.has(id),
    );
    const source = previous && type !== "delete" ? await read() : undefined;
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
    const codes = new Map<string, string | undefined>();
    for (const consumer of consumers) {
      let code: string | undefined;
      if (type === "update") {
        try {
          code = (await compile(name, consumer.code, consumer.id, host))?.code;
        } catch {
          // The transform that the hot update starts reports the failure.
        }
      }
      codes.set(consumer.id, code);
    }
    return {
      previous,
      source,
      result,
      consumers,
      codes,
    };
  };

  type CompiledChange = Awaited<ReturnType<typeof compileChange>>;

  return {
    /**
     * Starts the compilations of a changed file for the next hot update of this environment.
     *
     * Vite runs this hook before it invalidates the modules of the file, and runs the hot update
     * after the modules are invalidated. A request that arrives while a hot update compiles has to
     * transform the invalidated module, and the module runner of an SSR environment can reload
     * while that request still evaluates modules. The request then mixes module instances from
     * before and after the reload. For example, SvelteKit renders a page with a second instance
     * of the Svelte runtime and fails. A request that arrives while this hook compiles still uses
     * the modules from before the change.
     *
     * @param file - Changed file path.
     * @param change - Vite file event. Only `update` events start compilations.
     */
    async watchChange(file, change) {
      const name = this.environment?.name;
      const server = settings.devServerRef;
      const environment = name === undefined ? undefined : server?.environments[name];
      const id = cleanModuleId(file);
      pending.delete(id);
      if (
        change.event !== "update" ||
        !environment ||
        project.environment(name).designSystem.observes(resolve(file), false)
      ) {
        return;
      }
      const host: CssContext = {
        resolve: (specifier, importer, options) =>
          environment.pluginContainer.resolveId(specifier, importer, options),
        addWatchFile: (id: string) => {
          server?.watcher.add(id);
        },
        warn: (message: string) => {
          this.warn(message);
        },
      };
      // Reading the file waits until the project's watcher listener has discarded the shared
      // sources of the file, which it does in the same watcher event.
      const compiled = compileChange(name!, id, "update", () => readFile(file, "utf8"), host);
      pending.set(id, compiled);
      // The hot update reports a failure.
      await compiled.catch(() => {});
    },
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
        const prepared = context.type === "update" ? pending.get(id) : undefined;
        pending.delete(id);
        const { previous, source, result, consumers, codes } = await (prepared ??
          compileChange(name, id, context.type, () => context.read(), host));
        const stale = [];
        for (const consumer of consumers) {
          const code = codes.get(consumer.id);
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
