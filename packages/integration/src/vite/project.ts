import { createHash } from "node:crypto";
import { realpathSync } from "node:fs";
import { readFile, realpath } from "node:fs/promises";
import { dirname, isAbsolute, resolve } from "node:path";
import { pathToFileURL } from "node:url";

import {
  cssWithSourceMap,
  decodeCssSourceMap,
  loadCssGraph,
  compilerModuleId,
  cleanModuleId,
  fileSourceKind,
  moduleSourceMap,
  type CssUpstreamMap,
} from "@stillwind/host";
import type { NativeProjectSession } from "@stillwind/native";
import type { ViteDevServer } from "vite";

import { cssEntrypoint, compilerOptions, writeVocabulary } from "../options.ts";
import type { StillwindOptions as CommonOptions } from "../options.ts";
import { IntegrationSession } from "../session.ts";
import { browserSourceMap } from "./browser-source-map.ts";
import { readableFile } from "./file-access.ts";
import { finalizeModuleWithMap } from "./finalize.ts";
import { sourceModuleId } from "./module-ids.ts";
import { ModuleRegistry } from "./module-registry.ts";
import {
  resourceReplacements,
  resolvedFile,
  resolveResources,
  type StylesheetResource,
} from "./resources.ts";

export interface StillwindOptions extends CommonOptions {
  /**
   * Build transport using the ordered stylesheet plan.
   */
  emission?: "single" | "split";
}

/**
 * Creates project state and returns helpers for the Vite hooks and framework adapters.
 * `options` selects the CSS entrypoint and compiler options. Before serving requests,
 * callers must set project settings from Vite configuration and attach the server.
 * Each environment has its own compiler, inputs, and CSS revision. Compilation and
 * CSS publication run in order. `close` removes watch listeners and closes any
 * TypeScriptFactsService instances.
 */
export function createViteProject(options: StillwindOptions) {
  const settings = {
    watching: false,
    /**
     * Uses watcher events to detect CSS changes without reading every file on each JS edit.
     */
    cssWatching: false,
    compilerMode: "release",
    projectRoot: "",
    devServerRef: undefined as ViteDevServer | undefined,
    assetsDirectory: "assets",
    publicBase: "/",
    sourceMaps: false,
    devSourceMaps: false,
    minifyCss: true,
  };
  type EnvironmentState = {
    /**
     * Resolves when the last queued operation ends, even if that operation failed.
     */
    work: Promise<void>;
    session?: NativeProjectSession;
    resources: Map<string, StylesheetResource>;
    /**
     * Files in the last accepted CSS graph. With `watch: null`, their contents are hashed.
     */
    cssFiles: Set<string>;
    /**
     * Hash used to skip CSS reloads when the watcher is disabled and file contents match.
     */
    cssVersion?: string;
    /**
     * Absolute paths watched while loading CSS, including missing relative imports.
     * Keeping missing paths lets file creation trigger a retry after a load failure.
     */
    cssObservedFiles: Set<string>;
    /**
     * Counter increased when the watcher reports a CSS file added, changed, or removed.
     */
    cssChange: number;
    /**
     * Value of `cssChange` when the current CSS graph was accepted.
     */
    cssApplied: number;
    /**
     * Value of `cssChange` when loading was last tried, even if it failed. -1 means no attempt.
     * Keeping this value skips repeated failed loads until the watcher reports another change.
     */
    cssAttempted: number;
    /**
     * Last load or reload error. A successful load clears it.
     */
    cssError?: unknown;
    themeChanges: Set<string>;
    compiler?: IntegrationSession;
    revision: number;
    buildCss?: string;
    manifest?: {
      version: 1;
      revision: string;
      parts: { id: string; href: string }[];
      modules: Record<string, string[]>;
    };
  };
  const environments = new Map<string, EnvironmentState>();
  const environmentState = (name = "client") => {
    let state = environments.get(name);
    if (!state) {
      state = {
        work: Promise.resolve(),
        resources: new Map(),
        cssFiles: new Set(),
        cssObservedFiles: new Set(),
        cssChange: 0,
        cssApplied: 0,
        cssAttempted: -1,
        themeChanges: new Set(),
        revision: 0,
      };
      environments.set(name, state);
    }
    return state;
  };
  const closeTypeFacts = () => {
    for (const state of environments.values()) {
      state.compiler?.close();
    }
  };

  /**
   * Runs `operation` after earlier work in `state` and returns its result.
   * Errors reach the caller, but later operations can still run. `operation` must not
   * wait for another queued operation, because that operation would wait for it to finish.
   * Running operations in order avoids loading CSS twice or publishing conflicting snapshots.
   */
  const inTurn = async <T>(state: EnvironmentState, operation: () => Promise<T>) => {
    const previous = state.work;
    let release!: () => void;
    state.work = new Promise<void>((resolve) => {
      release = resolve;
    });
    await previous;
    try {
      return await operation();
    } finally {
      release();
    }
  };
  let stopCssWatching = () => {};
  /**
   * Replaces existing listeners with CSS listeners on `server`'s file watcher.
   * No HMR connection is needed. Closing the HTTP server or calling `close` removes
   * the listeners. If the watcher is disabled, requests check file contents instead.
   */
  const watchCssChanges = (server: ViteDevServer) => {
    stopCssWatching();
    settings.cssWatching = server.config.server.watch !== null;
    if (!settings.cssWatching) {
      return;
    }
    const changed = (event: string, file: string) => {
      if (event !== "add" && event !== "change" && event !== "unlink") {
        return;
      }
      const path = resolve(file);
      for (const state of environments.values()) {
        if (state.cssObservedFiles.has(path)) {
          state.cssChange++;
        }
      }
    };
    const stop = () => {
      server.watcher.off("all", changed);
      server.httpServer?.off("close", stop);
      settings.cssWatching = false;
    };
    server.watcher.on("all", changed);
    server.httpServer?.once("close", stop);
    stopCssWatching = stop;
  };
  const close = () => {
    stopCssWatching();
    closeTypeFacts();
    for (const registry of registries.values()) {
      registry.clear();
    }
  };

  const loadDesignSystem = async (
    resolveImport: (specifier: string, importer: string) => Promise<string>,
    dependencies: Set<string>,
    session?: NativeProjectSession,
    watch: (file: string) => void = () => {},
  ) => {
    const entry = resolve(settings.projectRoot, cssEntrypoint(options));
    watch(entry);
    const root = realpathSync(entry);
    const resolveCss = async (specifier: string, importer: string) => {
      if (specifier.startsWith(".")) {
        watch(resolve(dirname(importer), cleanModuleId(specifier)));
      }
      const file = await resolveImport(specifier, importer);
      watch(cleanModuleId(file));
      return file;
    };
    const nativeOptions = compilerOptions(options);
    const { loader, resources: resourceReferences } = await loadCssGraph(
      root,
      nativeOptions,
      {
        resolve: async (specifier, importer) => realpath(await resolveCss(specifier, importer)),
        read: (name) => readFile(name, "utf8"),
        documentUri: (name) => pathToFileURL(name).href,
        sourceMapSource: (name) => pathToFileURL(name).href,
        watch: (name) => {
          dependencies.add(name);
          watch(name);
        },
      },
      undefined,
      session?.newLoader(nativeOptions),
    );
    const references = new Map<string, StylesheetResource>();
    for (const resource of resourceReferences) {
      const resolved = await resolveCss(resource.specifier, resource.sourceName);
      const file = await readableFile(resolved, settings.devServerRef);
      references.set(resource.placeholder, {
        id: resolved,
        file,
      });
      dependencies.add(cleanModuleId(resolved));
    }
    return { loader, resources: references };
  };

  type CssContext = {
    resolve: (
      specifier: string,
      importer: string,
      options: { skipSelf: boolean },
    ) => Promise<{ id: string; external?: boolean | string } | null>;
    addWatchFile: (id: string) => void;
  };
  /**
   * Loads CSS through `context` and returns the loader, URL replacements, and file sets.
   * It leaves the active compiler revision unchanged. Files are watched before reading,
   * including paths for relative imports that cannot yet be resolved. Resolve and read
   * errors reject the promise, but watches stay registered so later changes can trigger a retry.
   */
  const loadRevision = async (state: EnvironmentState, context: CssContext) => {
    const dependencies = new Set<string>();
    const observed = new Set<string>();
    // Keep this set so reads that finish later cannot change the next revision's file set.
    const tracking = state.cssObservedFiles;
    const watch = (file: string) => {
      const path = resolve(file);
      if (observed.has(path)) {
        return;
      }
      observed.add(path);
      tracking.add(path);
      context.addWatchFile(path);
      settings.devServerRef?.watcher.add(path);
    };
    const result = await loadDesignSystem(
      async (specifier, importer) =>
        resolvedFile(
          await context.resolve(specifier, importer, { skipSelf: true }),
          `CSS ${specifier} from ${importer}`,
        ),
      dependencies,
      state.session,
      watch,
    );
    return { ...result, dependencies, observed };
  };
  /**
   * Creates a session if `state` has none. The caller must run this in `state`'s queue.
   * If the watcher reports a CSS change during loading, loading is retried before the
   * session is stored. Errors reject the promise. An existing session is left unchanged.
   */
  const ensureSessionInTurn = async (state: EnvironmentState, context: CssContext) => {
    if (state.session) {
      return;
    }
    while (true) {
      const change = state.cssChange;
      let loaded: Awaited<ReturnType<typeof loadRevision>>;
      try {
        loaded = await loadRevision(state, context);
      } catch (error) {
        if (settings.cssWatching && change !== state.cssChange) {
          continue;
        }
        throw error;
      }
      const version =
        settings.watching && !settings.cssWatching
          ? await designSystemVersion(loaded.dependencies)
          : undefined;
      if (settings.cssWatching && change !== state.cssChange) {
        continue;
      }
      state.compiler?.close();
      state.compiler = new IntegrationSession(
        settings.projectRoot,
        loaded.loader,
        options,
        settings.watching,
        settings.compilerMode,
      );
      state.session = state.compiler.native;
      state.resources = loaded.resources;
      state.cssFiles = loaded.dependencies;
      state.cssObservedFiles = loaded.observed;
      state.cssVersion = version;
      state.cssApplied = state.cssAttempted = change;
      state.cssError = undefined;
      state.revision++;
      writeVocabulary(loaded.loader, settings.projectRoot, options);
      return;
    }
  };
  const ensureSession = (state: EnvironmentState, context: CssContext) =>
    inTurn(state, () => ensureSessionInTurn(state, context));
  const startSession = (name: string | undefined, context: CssContext) => {
    const state = environmentState(name);
    return inTurn(state, async () => {
      state.session = undefined;
      inputMap(name).clear();
      moduleRegistry(name).clear();
      state.buildCss = undefined;
      state.manifest = undefined;
      await ensureSessionInTurn(state, context);
    });
  };

  /**
   * Returns a hash of sorted file paths and their contents for `watch: null`.
   * Reading 64 files at a time overlaps disk access without opening every file at once.
   * Missing files add a fixed marker to the hash. Other read errors reject the promise.
   */
  const designSystemVersion = async (files: Set<string>) => {
    const hash = createHash("sha256");
    const ordered = [...files].sort();
    for (let offset = 0; offset < ordered.length; offset += 64) {
      const batch = ordered.slice(offset, offset + 64);
      const contents = await Promise.all(
        batch.map(async (file) => {
          try {
            return await readFile(file);
          } catch (error) {
            if ((error as NodeJS.ErrnoException).code !== "ENOENT") {
              throw error;
            }
            return "\0missing";
          }
        }),
      );
      for (const [index, file] of batch.entries()) {
        hash.update(file).update("\0").update(contents[index]).update("\0");
      }
    }
    return hash.digest("hex");
  };

  /**
   * Reloads CSS in `state` and records producers whose transforms changed.
   * The caller must run this in `state`'s queue. Watcher counters skip file reads
   * until another CSS change is reported. Without a watcher, file hashes detect changes.
   * A CSS change during loading causes a retry. Errors from the first load reject the
   * promise. Later errors are stored and logged. Failed loads and reloads keep active output.
   */
  const refreshDesignSystemInTurn = async (state: EnvironmentState, context: CssContext) => {
    await ensureSessionInTurn(state, context);
    if (!settings.watching) {
      return;
    }
    while (true) {
      const change = state.cssChange;
      if (settings.cssWatching && state.cssAttempted === change) {
        return;
      }
      try {
        const version = settings.cssWatching
          ? undefined
          : await designSystemVersion(state.cssFiles);
        if (!settings.cssWatching && version === state.cssVersion) {
          return;
        }
        const loaded = await loadRevision(state, context);
        const nextVersion = settings.cssWatching
          ? undefined
          : await designSystemVersion(loaded.dependencies);
        if (
          settings.cssWatching
            ? change !== state.cssChange
            : version !== (await designSystemVersion(state.cssFiles))
        ) {
          continue;
        }
        const update = state.session!.reload(loaded.loader);
        const name = [...environments].find(([, candidate]) => candidate === state)?.[0];
        for (const module of update.transformedModules) {
          state.themeChanges.add(module);
        }
        const graph =
          name === "client"
            ? undefined
            : settings.devServerRef?.environments[name ?? "client"]?.moduleGraph;
        for (const input of inputMap(name).values()) {
          if (update.transformedModules.includes(input.moduleId)) {
            for (const module of graph?.getModulesByFile(cleanModuleId(input.id)) ?? []) {
              graph!.invalidateModule(module);
            }
          }
        }
        state.resources = loaded.resources;
        state.cssFiles = loaded.dependencies;
        state.cssObservedFiles = loaded.observed;
        state.cssVersion = nextVersion;
        state.cssApplied = state.cssAttempted = change;
        state.cssError = undefined;
        writeVocabulary(loaded.loader, settings.projectRoot, options);
        return;
      } catch (error) {
        if (settings.cssWatching && change !== state.cssChange) {
          continue;
        }
        state.cssAttempted = change;
        state.cssError = error;
        settings.devServerRef?.config.logger.warn(
          `Stillwind update failed; retaining published styles: ${String(error)}`,
        );
        return;
      }
    }
  };
  const refreshDesignSystem = (state: EnvironmentState, context: CssContext) =>
    inTurn(state, () => refreshDesignSystemInTurn(state, context));

  type Input = {
    projection?: boolean;
    upstream?: CssUpstreamMap;
    code: string;
    sourceMap?: ReturnType<typeof moduleSourceMap>;
    id: string;
    moduleId: string;
    manual: boolean;
    javascript: string;
    dependencies: string[];
  };
  const inputs = new Map<string, Map<string, Input>>();
  const inputMap = (name = "client") => {
    let map = inputs.get(name);
    if (!map) {
      inputs.set(name, (map = new Map()));
    }
    return map;
  };
  const publicSourceMap = (
    name: string | undefined,
    map: Parameters<typeof browserSourceMap>[0],
  ) => {
    const visible = new Map<string, string>();
    const add = (id: string) => {
      const relative = sourceModuleId(settings.projectRoot, cleanModuleId(id));
      if (relative !== ".." && !relative.startsWith("../") && !isAbsolute(relative)) {
        visible.set(pathToFileURL(cleanModuleId(id)).href, `stillwind-source:/${relative}`);
      }
    };
    for (const file of environmentState(name).cssFiles) {
      add(file);
    }
    if ((name ?? "client") === "client") {
      for (const input of inputMap(name).values()) {
        if (!input.projection && !/\.(?:astro|svelte|vue)$/.test(cleanModuleId(input.id))) {
          add(input.id);
        }
      }
    }
    const publicNames = new Set(visible.values());
    return browserSourceMap(map, (source) => {
      if (/^stillwind-source:[a-f0-9]{64}$/.test(source) || publicNames.has(source)) {
        return source;
      }
      return (
        visible.get(source) ??
        `stillwind-source:${createHash("sha256").update(source).digest("hex")}`
      );
    });
  };
  const cssForBrowser = (
    name: string | undefined,
    output: Parameters<typeof cssWithSourceMap>[0],
    replacements: ReadonlyMap<string, string>,
  ) => {
    const decoded = decodeCssSourceMap(
      cssWithSourceMap(
        output,
        replacements,
        [...inputMap(name).values()].flatMap((input) => (input.upstream ? [input.upstream] : [])),
        false,
      ),
    );
    if (!decoded.map) {
      return decoded.code;
    }
    const encoded = Buffer.from(JSON.stringify(publicSourceMap(name, decoded.map))).toString(
      "base64",
    );
    return `${decoded.code}\n/*# sourceMappingURL=data:application/json;base64,${encoded} */`;
  };
  const registries = new Map<string, ModuleRegistry>();
  const moduleRegistry = (name = "client") => {
    let registry = registries.get(name);
    if (!registry) {
      registry = new ModuleRegistry(settings.projectRoot, name);
      registries.set(name, registry);
    }
    return registry;
  };
  const producerSpecifier = (id: string, name?: string) =>
    moduleRegistry(name).source("producer", id);
  const valueSpecifier = (id: string, name?: string) => moduleRegistry(name).source("value", id);
  const resolvePlan = async (
    state: EnvironmentState,
    modules: string[],
    context: {
      resolve: (
        specifier: string,
        importer: string,
        options: { skipSelf: boolean },
      ) => Promise<{ id: string; external?: boolean | string } | null>;
      addWatchFile: (id: string) => void;
    },
    complete = false,
  ) => {
    const revision = state.revision;
    const plan = state.session!.planStylesheet(
      {
        modules,
        coverage: complete
          ? {
              kind: "complete",
              target: "application",
              revision: String(state.revision),
            }
          : { kind: "observed" },
        placement: "document",
      },
      settings.watching ? settings.devSourceMaps : settings.sourceMaps,
    );
    const resources = new Map(state.resources);
    for (const resource of plan.resources) {
      const resolved = await context.resolve(
        resource.specifier,
        state.compiler!.resolverId(resource.moduleId),
        {
          skipSelf: true,
        },
      );
      const id = resolvedFile(resolved, `Stillwind resource ${resource.specifier}`);
      const file = await readableFile(id, settings.devServerRef);
      context.addWatchFile(file);
      resources.set(resource.placeholder, {
        id,
        file,
      });
    }
    const replacements = resourceReplacements(resources);
    return {
      ...plan,
      revision,
      originalCss: plan.css,
      resources,
      resourceReplacements: replacements,
      css: resolveResources(plan.css, replacements),
      rawParts: plan.parts,
      parts: plan.parts.map((part) => {
        const name = [...environments].find(([, candidate]) => candidate === state)?.[0];
        const mapped =
          settings.watching && settings.devSourceMaps
            ? decodeCssSourceMap(cssForBrowser(name, part, replacements))
            : undefined;
        return {
          key: part.key,
          producers: part.producers,
          css: mapped?.code ?? resolveResources(part.css, replacements),
          map: mapped?.map,
        };
      }),
    };
  };
  const documentPlanInTurn = async (
    name: string | undefined,
    context: Parameters<typeof resolvePlan>[2],
    complete = false,
    selection?: readonly string[],
  ) => {
    const state = environmentState(name);
    await refreshDesignSystemInTurn(state, context);
    const modules = [...inputMap(name).values()]
      .filter((input) => !input.manual)
      .filter((input) => !selection || selection.includes(cleanModuleId(input.moduleId)))
      .map((input) => input.moduleId);
    return resolvePlan(state, modules, context, complete);
  };
  const documentPlan = (
    name: string | undefined,
    context: CssContext,
    complete = false,
    selection?: readonly string[],
  ) => inTurn(environmentState(name), () => documentPlanInTurn(name, context, complete, selection));
  const publish = async (name: string | undefined, context: Parameters<typeof resolvePlan>[2]) => {
    if (!settings.watching || !settings.devServerRef) {
      return;
    }
    const state = environmentState(name);
    await inTurn(state, async () => {
      await refreshDesignSystemInTurn(state, context);
      const revision = ++state.revision;
      const modules = [...inputMap(name).values()]
        .filter((input) => !input.manual)
        .map((input) => input.moduleId);
      const plan = await resolvePlan(state, modules, context);
      if (revision === state.revision) {
        settings.devServerRef?.environments[name ?? "client"]?.hot.send({
          type: "custom",
          event: "stillwind:css",
          data: {
            revision,
            parts: plan.parts,
            dependencies: Object.fromEntries(
              [...inputMap(name).values()].map((input) => [input.moduleId, input.dependencies]),
            ),
          },
        });
      }
    });
  };
  const compile = async (
    name: string | undefined,
    code: string,
    id: string,
    context: Parameters<typeof resolvePlan>[2] & {
      warn: (message: string) => void;
      getCombinedSourcemap?: () => unknown;
    },
  ) => {
    const state = environmentState(name);
    return inTurn(state, async () => {
      await refreshDesignSystemInTurn(state, context);
      const sfc = /\.(?:astro|svelte|vue)$/.test(cleanModuleId(id));
      const documentUri = (id: string) => {
        const clean = cleanModuleId(id);
        return clean.startsWith("\0") || clean.startsWith("virtual:")
          ? `stillwind-virtual:${encodeURIComponent(clean)}`
          : pathToFileURL(clean).href;
      };
      const output = await state.compiler!.compile(
        code,
        id,
        {
          resolve: (specifier, importer) =>
            context.resolve(specifier, importer, { skipSelf: true }),
          read: (id) => readFile(cleanModuleId(id), "utf8"),
          describe: (resolvedId) => ({
            moduleId:
              resolvedId.startsWith("\0") || resolvedId.startsWith("virtual:")
                ? `virtual:${encodeURIComponent(resolvedId)}`
                : sourceModuleId(settings.projectRoot, resolvedId),
            document: {
              uri: documentUri(resolvedId),
              format: fileSourceKind(resolvedId).format,
            },
            checkerFileName:
              resolvedId.startsWith("\0") || resolvedId.startsWith("virtual:")
                ? resolve(
                    settings.projectRoot,
                    ".stillwind-checker",
                    `${createHash("sha256").update(resolvedId).digest("hex")}.ts`,
                  )
                : cleanModuleId(resolvedId),
            projectionFileName:
              resolvedId.startsWith("\0") || resolvedId.startsWith("virtual:")
                ? `virtual/${createHash("sha256").update(resolvedId).digest("hex")}.ts`
                : compilerModuleId(settings.projectRoot, cleanModuleId(resolvedId)),
            syntax: fileSourceKind(resolvedId).syntax,
          }),
          watch: (id) => context.addWatchFile(id),
          warn: (message) => context.warn(message),
        },
        {
          stylesheetImports:
            sfc && options.automatic !== false ? [producerSpecifier(id, name)] : undefined,
          moduleCssImport: valueSpecifier(id, name),
        },
      );
      if (!output?.code) {
        return;
      }
      const finalized = sfc
        ? { code: output.code, removed: [], prefix: "" }
        : finalizeModuleWithMap(output.code, false);
      const suffix =
        sfc || output.hasModuleCssImport || options.automatic === false
          ? ""
          : `\nimport ${JSON.stringify(producerSpecifier(id, name))};\n`;
      const javascript = finalized.code + suffix;
      const sourceMap = moduleSourceMap(
        code,
        output.code,
        id,
        output.mappings,
        finalized.removed,
        finalized.prefix,
      );
      inputMap(name).set(id, {
        sourceMap,
        projection: inputMap(name).get(id)?.projection,
        upstream: context.getCombinedSourcemap
          ? {
              id: cleanModuleId(id),
              sourceMapSource: documentUri(id),
              map: JSON.stringify(context.getCombinedSourcemap()),
            }
          : inputMap(name).get(id)?.upstream,
        code,
        id,
        moduleId: output.moduleId,
        manual: output.hasModuleCssImport || options.automatic === false,
        javascript,
        dependencies: output.compositionModules,
      });
      return {
        code: javascript,
        map: JSON.stringify(
          moduleSourceMap(
            code,
            output.code,
            id,
            output.mappings,
            finalized.removed,
            finalized.prefix,
          ),
        ),
        meta: {
          stillwind: {
            input: code,
            id,
            moduleId: output.moduleId,
            manual: output.hasModuleCssImport,
          },
        },
      };
    });
  };

  return {
    options,
    settings,
    environmentState,
    closeTypeFacts,
    close,
    watchCssChanges,
    startSession,
    loadDesignSystem,
    ensureSession,
    refreshDesignSystem,
    inputMap,
    publicSourceMap,
    cssForBrowser,
    moduleRegistry,
    producerSpecifier,
    valueSpecifier,
    resolvePlan,
    documentPlan,
    publish,
    compile,
  };
}

export type ViteProject = ReturnType<typeof createViteProject>;
