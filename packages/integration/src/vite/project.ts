import { createHash } from "node:crypto";
import { realpathSync } from "node:fs";
import { readFile, realpath } from "node:fs/promises";
import { resolve } from "node:path";
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
  /** Build transport using the ordered stylesheet plan. */
  emission?: "single" | "split";
}

export function createViteProject(options: StillwindOptions) {
  const settings = {
    watching: false,
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
    loadRevision: number;
    session?: NativeProjectSession;
    initializing?: Promise<void>;
    resources: Map<string, string>;
    cssFiles: Set<string>;
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
        loadRevision: 0,
        resources: new Map(),
        cssFiles: new Set(),
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

  const loadDesignSystem = async (
    resolveImport: (specifier: string, importer: string) => Promise<string>,
    dependencies: Set<string>,
    session?: NativeProjectSession,
  ) => {
    const root = realpathSync(resolve(settings.projectRoot, cssEntrypoint(options)));
    const nativeOptions = compilerOptions(options);
    const { loader, resources: resourceReferences } = await loadCssGraph(
      root,
      nativeOptions,
      {
        resolve: async (specifier, importer) => realpath(await resolveImport(specifier, importer)),
        read: (name) => readFile(name, "utf8"),
        documentUri: (name) => pathToFileURL(name).href,
        sourceMapSource: (name) => pathToFileURL(name).href,
        watch: (name) => dependencies.add(name),
      },
      undefined,
      session?.newLoader(nativeOptions),
    );
    const references = new Map<string, string>();
    for (const resource of resourceReferences) {
      const resolved = await resolveImport(resource.specifier, resource.sourceName);
      const url = resourceUrl(resolved);
      references.set(resource.placeholder, JSON.stringify(url).slice(1, -1));
      dependencies.add(cleanModuleId(resolved));
    }
    return { loader, resources: references };
  };

  const ensureSession = async (
    state: EnvironmentState,
    context: {
      resolve: (
        specifier: string,
        importer: string,
        options: { skipSelf: boolean },
      ) => Promise<{ id: string; external?: boolean | string } | null>;
      addWatchFile: (id: string) => void;
    },
  ) => {
    if (state.session) {
      return;
    }
    if (!state.initializing) {
      const revision = ++state.revision;
      const initialize = async () => {
        const dependencies = new Set<string>();
        const { loader, resources } = await loadDesignSystem(async (specifier, importer) => {
          const result = await context.resolve(specifier, importer, {
            skipSelf: true,
          });
          return resolvedFile(result, `CSS ${specifier} from ${importer}`);
        }, dependencies);
        if (state.revision !== revision) {
          return;
        }
        state.compiler?.close();
        state.compiler = new IntegrationSession(
          settings.projectRoot,
          loader,
          options,
          settings.watching,
          settings.compilerMode,
        );
        state.session = state.compiler.native;
        state.resources = resources;
        state.cssFiles = dependencies;
        for (const path of dependencies) {
          context.addWatchFile(path);
        }
        writeVocabulary(loader, settings.projectRoot, options);
      };
      state.initializing = initialize().finally(() => {
        state.initializing = undefined;
      });
    }
    await state.initializing;
  };

  type Input = {
    projection?: boolean;
    upstream?: CssUpstreamMap;
    code: string;
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
    const plan = state.session!.planStylesheet({
      modules,
      coverage: complete
        ? {
            kind: "complete",
            target: "application",
            revision: String(state.revision),
          }
        : { kind: "observed" },
      placement: "document",
    });
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
  const documentPlan = async (
    name: string | undefined,
    context: Parameters<typeof resolvePlan>[2],
    complete = false,
    selection?: readonly string[],
  ) => {
    const state = environmentState(name);
    await ensureSession(state, context);
    const modules = [...inputMap(name).values()]
      .filter((input) => !input.manual)
      .filter((input) => !selection || selection.includes(cleanModuleId(input.moduleId)))
      .map((input) => input.moduleId);
    return resolvePlan(state, modules, context, complete);
  };
  const publish = async (name: string | undefined, context: Parameters<typeof resolvePlan>[2]) => {
    if (!settings.watching || !settings.devServerRef) {
      return;
    }
    const state = environmentState(name);
    const revision = ++state.revision;
    const plan = await documentPlan(name, context);
    if (revision === state.revision) {
      settings.devServerRef.environments[name ?? "client"]?.hot.send({
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
    const previous = state.work;
    let release!: () => void;
    state.work = new Promise<void>((resolve) => {
      release = resolve;
    });
    await previous;
    try {
      await ensureSession(state, context);
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
      inputMap(name).set(id, {
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
    } finally {
      release();
    }
  };

  return {
    options,
    settings,
    environmentState,
    closeTypeFacts,
    loadDesignSystem,
    ensureSession,
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
