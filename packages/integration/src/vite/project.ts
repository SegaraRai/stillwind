import { resolve } from "node:path";

import type { EncodedSourceMap } from "@jridgewell/gen-mapping";
import { cleanModuleId, type CssSourceMapInput } from "@stillwind/host";
import type { InferCustomEventPayload, ResolvedConfig, ViteDevServer } from "vite";

import type { StillwindOptions as CommonOptions } from "../options.ts";
import { cssForBrowser, publicCssSourceMap } from "./browser-css.ts";
import type { BrowserSourceMapInput } from "./browser-source-map.ts";
import type { BuildOutput } from "./build.ts";
import type { CompileContext, ProducerTransform } from "./compile.ts";
import type { CssContext } from "./design-system.ts";
import { ViteEnvironment, type CssSnapshot } from "./environment.ts";
import type { StylesheetPlan } from "./stylesheet.ts";

/** Compiler configuration for Vite and adapters that share its stylesheet emission. */
export interface StillwindOptions extends CommonOptions {
  /**
   * Production CSS placement. `single` emits one document stylesheet. `split` emits ordered parts
   * selected by chunks. Defaults to `single`.
   */
  emission?: "single" | "split";
}

/** Configuration shared by all compiler environments in one Vite integration instance. */
export interface ProjectSettings {
  /** Whether the Vite command is `serve`. This selects development compilation and CSS delivery. */
  watching: boolean;
  /**
   * Whether the attached server has an active CSS watcher. Otherwise development detects CSS
   * changes through file fingerprints.
   */
  cssWatching: boolean;
  /** Native compilation mode: `development` for serving and `release` for production builds. */
  compilerMode: string;
  /** Absolute project root after configuration, including the optional compiler root override. */
  projectRoot: string;
  /** Active development server used for resource checks, file watching, and CSS hot messages. */
  devServerRef?: ViteDevServer;
  /**
   * Build asset directory copied from Vite, used to place CSS files and compute relative resource
   * URLs.
   */
  assetsDirectory: string;
  /** Public URL base copied from Vite, including its trailing slash or relative-base form. */
  publicBase: string;
  /** Whether production CSS should carry source maps, derived from `build.sourcemap`. */
  sourceMaps: boolean;
  /** Whether development CSS should carry source maps, derived from `css.devSourcemap`. */
  devSourceMaps: boolean;
  /**
   * Whether emitted production CSS is minified. Disabled only when Vite sets `build.cssMinify` to
   * false.
   */
  minifyCss: boolean;
}

/**
 * Shared project operations that route compilation and stylesheet work to isolated Vite
 * environments.
 */
export interface ViteProject {
  /** Compiler options shared by all environments for this integration instance. */
  options: StillwindOptions;
  /** Mutable settings initialized by the compiler plugin before environment startup. */
  settings: ProjectSettings;
  /**
   * Obtains isolated compilation state for a Vite environment, creating it on first use.
   *
   * @param name - Environment name. Omission selects `client`.
   * @returns The environment retained for this project lifetime.
   */
  environment: (name?: string) => ViteEnvironment;
  /**
   * Updates shared settings from resolved Vite configuration and closes existing type facts.
   *
   * @param config - Resolved configuration for this project. Call before starting environments.
   */
  configure: (config: ResolvedConfig) => void;
  /**
   * Attaches a development server and replaces any previously registered CSS watch listener.
   *
   * @param server - Active Vite server. A null watch configuration disables event tracking.
   */
  attachServer: (server: ViteDevServer) => void;
  /** Closes type-facts services in existing environments while retaining native compiler state. */
  closeTypeFacts: () => void;
  /** Removes CSS watch listeners, closes type facts, and clears environment virtual references. */
  close: () => void;
  /**
   * Obtains recorded producer inputs for one environment.
   *
   * @param name - Environment name. Omission selects `client`.
   * @returns The live map keyed by full Vite source identity. Callers must not mutate entries.
   */
  inputs: (name?: string) => ViteEnvironment["inputs"];
  /**
   * Obtains virtual references owned by one environment.
   *
   * @param name - Environment name. Omission selects `client`.
   * @returns The environment registry, created using the configured project root on first use.
   */
  moduleRegistry: (name?: string) => ViteEnvironment["registry"];
  /**
   * Registers the virtual producer module that activates automatic document styles.
   *
   * @param id - Full Vite source identity of the producer.
   * @param name - Environment name. Omission selects `client`.
   * @returns A registered `virtual:stillwind/producer:` specifier.
   */
  producerSpecifier: (id: string, name?: string) => string;
  /**
   * Registers the virtual module supplying an explicit CSS value for a producer.
   *
   * @param id - Full Vite source identity of the producer.
   * @param name - Environment name. Omission selects `client`.
   * @returns A registered `virtual:stillwind/value:` specifier.
   */
  valueSpecifier: (id: string, name?: string) => string;
  /**
   * Compiles and records a producer in its environment.
   *
   * @param name - Environment name, or `undefined` for `client`.
   * @param code - Source text before Stillwind transformation.
   * @param id - Full Vite source identity, including its query suffix.
   * @param context - Resolution, watch, warning, and optional upstream-map operations.
   * @returns A promise for the transform result, or `undefined` when no code was produced.
   */
  compile: (
    name: string | undefined,
    code: string,
    id: string,
    context: CompileContext,
  ) => Promise<ProducerTransform | undefined>;
  /**
   * Plans document CSS for automatic producers in one environment.
   *
   * @param name - Environment name, or `undefined` for `client`.
   * @param context - Resolver and watch operations for referenced resources.
   * @param complete - Whether the caller has collected the complete application graph.
   * @param selection - Optional compiler module paths without queries. Omission selects all
   * automatic inputs. An empty list selects none.
   * @returns A promise for resolved document CSS and ordered stylesheet parts.
   */
  documentPlan: (
    name: string | undefined,
    context: CssContext,
    complete?: boolean,
    selection?: readonly string[],
  ) => Promise<StylesheetPlan>;
  /**
   * Obtains document parts and producer dependencies at the current environment revision.
   *
   * @param name - Environment name, or `undefined` for `client`.
   * @param context - Resolver and watch operations for stylesheet planning.
   * @returns A promise for the current development snapshot without advancing its revision.
   */
  snapshot: (name: string | undefined, context: CssContext) => Promise<CssSnapshot>;
  /**
   * Publishes a new stylesheet snapshot through the environment's hot channel.
   *
   * @param name - Environment name, or `undefined` for `client`.
   * @param context - Resolver and watch operations for stylesheet planning.
   * @returns A promise that resolves after publication. Production and unattached servers return
   * without planning or sending a snapshot.
   */
  publish: (name: string | undefined, context: CssContext) => Promise<void>;
  /**
   * Rewrites CSS map sources for browser delivery using the selected environment's sources.
   *
   * @param name - Environment name, or `undefined` for `client`.
   * @param map - Encoded or serialized CSS source map.
   * @returns A map with approved or opaque source names and no embedded source text.
   */
  publicSourceMap: (name: string | undefined, map: BrowserSourceMapInput) => EncodedSourceMap;
  /**
   * Replaces CSS resource placeholders and composes browser-safe upstream maps.
   *
   * @param name - Environment name, or `undefined` for `client`.
   * @param output - Compiler CSS and its native source map.
   * @param replacements - Resource placeholder tokens mapped to escaped URL text.
   * @returns Resolved CSS with an inline public source map when a map is available.
   */
  cssForBrowser: (
    name: string | undefined,
    output: CssSourceMapInput,
    replacements: ReadonlyMap<string, string>,
  ) => string;
  /**
   * Obtains stylesheet assets and metadata from the latest production emission.
   *
   * @param name - Environment name. Omission selects `client`.
   * @returns Recorded build output, or an empty object if no stylesheet has been emitted.
   */
  buildOutput: (name?: string) => BuildOutput;
  /**
   * Replaces stylesheet assets and metadata for one environment.
   *
   * @param name - Environment name, or `undefined` for `client`.
   * @param output - Emitted references and manifest produced by the build hooks.
   */
  setBuildOutput: (name: string | undefined, output: BuildOutput) => void;
}

/**
 * Creates project state shared by compiler, development, and build plugins.
 *
 * Call `configure` before environment initialization, then `attachServer` when serving. Close the
 * project when its build or server lifetime ends.
 *
 * @param options - Compiler configuration and production CSS emission mode.
 * @returns An unconfigured project whose environments are created lazily.
 */
export function createViteProject(options: StillwindOptions): ViteProject {
  const settings: ProjectSettings = {
    watching: false,
    cssWatching: false,
    compilerMode: "release",
    projectRoot: "",
    assetsDirectory: "assets",
    publicBase: "/",
    sourceMaps: false,
    devSourceMaps: false,
    minifyCss: true,
  };

  const environments = new Map<string, ViteEnvironment>();
  const environment = (name = "client") => {
    let state = environments.get(name);
    if (!state) {
      state = new ViteEnvironment(name, settings, options, (ids) => {
        if (name === "client") {
          return;
        }
        const graph = settings.devServerRef?.environments[name]?.moduleGraph;
        for (const id of ids) {
          for (const module of graph?.getModulesByFile(cleanModuleId(id)) ?? []) {
            graph!.invalidateModule(module);
          }
        }
      });
      environments.set(name, state);
    }
    return state;
  };

  const closeTypeFacts = () => {
    for (const state of environments.values()) {
      state.closeTypeFacts();
    }
  };

  const configure = (config: ResolvedConfig) => {
    settings.projectRoot = resolve(config.root, options.root ?? ".");
    settings.assetsDirectory = config.build.assetsDir;
    settings.publicBase = config.base;
    settings.sourceMaps = Boolean(config.build.sourcemap);
    settings.devSourceMaps = Boolean(config.css.devSourcemap);
    settings.minifyCss = config.build.cssMinify !== false;
    settings.watching = config.command === "serve";
    settings.compilerMode = settings.watching ? "development" : "release";
    closeTypeFacts();
  };

  let stopCssWatching = () => {};

  const attachServer = (server: ViteDevServer) => {
    settings.devServerRef = server;
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
        state.changed(path);
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
    for (const state of environments.values()) {
      state.close();
    }
  };

  const publish = (name: string | undefined, context: CssContext) => {
    if (!settings.watching || !settings.devServerRef) {
      return Promise.resolve();
    }
    return environment(name).publish(context, (snapshot) => {
      settings.devServerRef?.environments[name ?? "client"]?.hot.send({
        type: "custom",
        event: "stillwind:css",
        data: snapshot satisfies InferCustomEventPayload<"stillwind:css">,
      });
    });
  };

  return {
    options,
    settings,
    environment,
    configure,
    attachServer,
    closeTypeFacts,
    close,
    inputs: (name?: string) => environment(name).inputs,
    moduleRegistry: (name?: string) => environment(name).registry,
    producerSpecifier: (id: string, name?: string) =>
      environment(name).registry.source("producer", id),
    valueSpecifier: (id: string, name?: string) => environment(name).registry.source("value", id),
    compile: (name: string | undefined, code: string, id: string, context: CompileContext) =>
      environment(name).compile(code, id, context),
    documentPlan: (
      name: string | undefined,
      context: CssContext,
      complete = false,
      selection?: readonly string[],
    ) => environment(name).documentPlan(context, complete, selection),
    snapshot: (name: string | undefined, context: CssContext) =>
      environment(name).snapshot(context),
    publish,
    publicSourceMap: (name: string | undefined, map: BrowserSourceMapInput) =>
      publicCssSourceMap(environment(name).browserSources, map),
    cssForBrowser: (
      name: string | undefined,
      output: CssSourceMapInput,
      replacements: ReadonlyMap<string, string>,
    ) => cssForBrowser(environment(name).browserSources, output, replacements),
    buildOutput: (name?: string): BuildOutput => environment(name).buildOutput ?? {},
    setBuildOutput: (name: string | undefined, output: BuildOutput) => {
      environment(name).buildOutput = output;
    },
  };
}
