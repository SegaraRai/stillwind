import { createHash } from "node:crypto";
import { realpathSync } from "node:fs";
import { readFile, realpath } from "node:fs/promises";
import { dirname, resolve } from "node:path";
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
import { finalizeModuleWithMap } from "./finalize.ts";
import { sourceModuleId } from "./module-ids.ts";
import { resourceUrl, resolvedFile, resolveResources } from "./resources.ts";

export interface StillwindOptions extends CommonOptions {
  /**
   * Build transport using the ordered stylesheet plan.
   */
  emission?: "single" | "split";
}

export function createViteProject(options: StillwindOptions) {
  const settings = {
    watching: false,
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
    work: Promise<void>;
    session?: NativeProjectSession;
    resources: Map<string, string>;
    cssFiles: Set<string>;
    cssVersion?: string;
    cssObservedFiles: Set<string>;
    cssChange: number;
    cssApplied: number;
    cssAttempted: number;
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
    const references = new Map<string, string>();
    for (const resource of resourceReferences) {
      const resolved = await resolveCss(resource.specifier, resource.sourceName);
      const url = resourceUrl(resolved);
      references.set(resource.placeholder, JSON.stringify(url).slice(1, -1));
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
  const loadRevision = async (state: EnvironmentState, context: CssContext) => {
    const dependencies = new Set<string>();
    const observed = new Set<string>();
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
      state.buildCss = undefined;
      state.manifest = undefined;
      await ensureSessionInTurn(state, context);
    });
  };

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
  const includeSourceContent = (name?: string) =>
    ![...inputMap(name).values()].some((input) => input.projection);
  const fragments = new Map<string, string>();
  const fragmentSpecifier = (css: string) => {
    const key = settings.watching
      ? createHash("sha256").update(css).digest("hex")
      : Buffer.from(css).toString("base64url");
    if (settings.watching) {
      fragments.set(key, css);
    }
    return "virtual:stillwind/fragment:" + key;
  };
  const producerSpecifier = (id: string) =>
    `virtual:stillwind/producer:${Buffer.from(sourceModuleId(settings.projectRoot, id)).toString("base64url")}`;
  const valueSpecifier = (id: string) =>
    `virtual:stillwind/value:${Buffer.from(id).toString("base64url")}`;
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
        { skipSelf: true },
      );
      const file = resolvedFile(resolved, `Stillwind resource ${resource.specifier}`);
      context.addWatchFile(file);
      resources.set(resource.placeholder, JSON.stringify(resourceUrl(file)).slice(1, -1));
    }
    return {
      ...plan,
      revision,
      originalCss: plan.css,
      resourceReplacements: resources,
      css: resolveResources(plan.css, resources),
      rawParts: plan.parts,
      parts: plan.parts.map((part) => {
        const name = [...environments].find(([, candidate]) => candidate === state)?.[0];
        const mapped =
          settings.watching && settings.devSourceMaps
            ? decodeCssSourceMap(
                cssWithSourceMap(
                  part,
                  resources,
                  [...inputMap(name).values()].flatMap((input) =>
                    input.upstream ? [input.upstream] : [],
                  ),
                  includeSourceContent(name),
                ),
              )
            : undefined;
        return {
          key: part.key,
          producers: part.producers,
          css: mapped?.code ?? resolveResources(part.css, resources),
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
            sfc && options.automatic !== false ? [producerSpecifier(id)] : undefined,
          moduleCssImport: valueSpecifier(id),
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
          : `\nimport ${JSON.stringify(producerSpecifier(id))};\n`;
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
    includeSourceContent,
    fragments,
    fragmentSpecifier,
    producerSpecifier,
    valueSpecifier,
    resolvePlan,
    documentPlan,
    publish,
    compile,
  };
}

export type ViteProject = ReturnType<typeof createViteProject>;
