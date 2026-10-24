import { watch } from "node:fs";
import { dirname, resolve } from "node:path";

import type { EncodedSourceMap } from "@jridgewell/gen-mapping";
import type { InlineConfig } from "@stillwind/config";
import { resolveIntegrationConfig } from "@stillwind/config/node";
import { cleanModuleId, type CssSourceMapInput } from "@stillwind/host";
import type { NativeCssMinifyOptions } from "@stillwind/native";
import type { Environment, InferCustomEventPayload, ResolvedConfig, ViteDevServer } from "vite";

import { cssForBrowser, publicCssSourceMap } from "./browser-css.ts";
import type { BrowserSourceMapInput } from "./browser-source-map.ts";
import type { BuildOutput } from "./build.ts";
import type { CompileContext, ProducerTransform } from "./compile.ts";
import { inheritCssTarget, minifyTargets } from "./css-targets.ts";
import type { CssContext } from "./design-system.ts";
import { ViteEnvironment, type CssSnapshot } from "./environment.ts";
import type { StylesheetPlan } from "./stylesheet.ts";

/** Vite environment instance whose identity and top-level configuration own compiler state. */
export type EnvironmentOwner = Pick<Environment, "name" | "getTopLevelConfig">;

/** File selection and compiler overrides for Vite, Astro, and Nuxt, with CSS emission placement. */
export interface IntegrationConfig extends InlineConfig {
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
  /** Browser versions for final CSS processing. Omission preserves unspecified compiler targets. */
  cssTargets?: NativeCssMinifyOptions["targets"];
}

/**
 * Shared project operations that route compilation and stylesheet work to isolated Vite
 * environments.
 */
export interface ViteProject {
  /**
   * Obtains the project owned by a resolved Vite configuration.
   * @param config - Top-level configuration, obtained from a server or getTopLevelConfig().
   * @returns This project for its first configuration, or an isolated project for another configuration.
   */
  forConfig: (config: ResolvedConfig) => ViteProject;
  /**
   * Evaluated compiler settings shared by all environments. Empty before configure. Reconfiguration
   * replaces the fields of this same object so plugin hooks retain the current settings.
   */
  config: IntegrationConfig;
  /** Mutable settings initialized by the compiler plugin before environment startup. */
  settings: ProjectSettings;
  /**
   * Obtains isolated compilation state for a Vite environment, creating it on first use.
   *
   * @param name - Owning Vite instance, or a name selecting the current state. Omission selects `client`.
   * @returns The environment retained for this project lifetime.
   */
  environment: (name?: string | EnvironmentOwner) => ViteEnvironment;
  /**
   * Closes the state owned by an environment without closing other environments.
   * @param environment - The owning Vite instance. Development also removes its state from watch tracking.
   */
  closeEnvironment: (environment: EnvironmentOwner) => void;
  /**
   * Evaluates Stillwind settings and replaces all existing compiler environments.
   *
   * @param config - Resolved configuration for this project. Call before starting environments.
   * @returns A promise that resolves after settings and configuration watch paths are updated.
   * Previously obtained environments must not be used after successful reconfiguration.
   * @throws If configuration evaluation or closing an existing environment fails.
   */
  configure: (config: ResolvedConfig) => Promise<void>;
  /** Absolute configuration dependencies and missing discovery candidates. Callers must not mutate this set. */
  configurationFiles: Set<string>;
  /**
   * Marks build configuration dirty when a watched path can affect its evaluation.
   *
   * @param id - Changed filesystem path, resolved to an absolute path for comparison.
   * @returns Whether the path belongs to configurationFiles. Other changes have no effect.
   */
  configurationChanged: (id: string) => boolean;
  /**
   * Reevaluates settings when configurationChanged marked them dirty and configure has already run.
   *
   * @returns A promise that resolves after reconfiguration, or immediately when no refresh is needed.
   * @throws If configuration evaluation or environment cleanup fails.
   */
  refreshConfiguration: () => Promise<void>;
  /**
   * Attaches a development server and replaces any previously registered CSS watch listener.
   *
   * @param server - Active Vite server. Call after configure so watch paths are known. A null watch
   * configuration disables event tracking. Configuration changes restart the server. CSS changes
   * invalidate existing environments.
   * @throws If a directory watcher for a missing configuration candidate cannot be created.
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
 * @param supplied - Configuration discovery, compiler overrides, and production CSS emission mode.
 * Omission reads stillwind.config.* when configure runs.
 * @param applicationRoot - Called during each configuration evaluation. Its directory overrides
 * Vite's root as the base for discovery and inline paths. Undefined uses Vite's root.
 * @returns An unconfigured project whose environments are created lazily.
 */
export function createViteProject(
  supplied: IntegrationConfig = {},
  applicationRoot: () => string | undefined = () => undefined,
): ViteProject {
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
  const config: IntegrationConfig = {};
  const generations = new WeakMap<ResolvedConfig, ViteProject>();
  let claimed = false;
  const configurationFiles = new Set<string>();
  let resolvedViteConfig: ResolvedConfig | undefined;
  let configurationDirty = false;
  let configurationDependencies: string[] = [];
  let missingConfigurations: string[] = [];

  const environments = new Map<string, ViteEnvironment>();
  let bindings = new WeakMap<EnvironmentOwner, ViteEnvironment>();
  /**
   * Gets or creates the compiler state owned by one Vite environment.
   * @param owner - Owning Vite instance, or a name selecting the current state. Omission selects `client`.
   * @returns The retained environment state. Newly created non-client states invalidate their
   * affected modules in the corresponding development module graph.
   */
  const environment = (owner: string | EnvironmentOwner = "client") => {
    const name = typeof owner === "string" ? owner : owner.name;
    let state = typeof owner === "string" ? environments.get(name) : bindings.get(owner);
    if (!state) {
      state = new ViteEnvironment(name, settings, config, (ids) => {
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
      if (typeof owner !== "string") {
        bindings.set(owner, state);
      }
    }
    return state;
  };

  const closeEnvironment = (owner: EnvironmentOwner) => {
    const state = bindings.get(owner);
    if (!state) {
      return;
    }
    state.close();
    if (settings.watching && environments.get(owner.name) === state) {
      environments.delete(owner.name);
    }
  };

  /** Releases checker services in all currently retained compiler environments. */
  const closeTypeFacts = () => {
    for (const state of environments.values()) {
      state.closeTypeFacts();
    }
  };

  /**
   * Replaces compiler environments and applied settings after resolving project configuration.
   * @param viteConfig - Resolved Vite configuration supplying root, asset, source-map, and watch settings.
   * @returns A promise that resolves after old environments close and configuration dependencies
   * and missing paths replace the previous discovery state.
   * @throws If the emission mode is invalid, project configuration fails, or an old environment cannot close.
   */
  const configure = async (viteConfig: ResolvedConfig) => {
    const { emission, ...inline } = supplied;
    if (emission !== undefined && emission !== "single" && emission !== "split") {
      throw new Error("config.emission: must be single or split");
    }
    const cssTarget = viteConfig.environments.client?.build.cssTarget ?? viteConfig.build.cssTarget;
    const resolved = await resolveIntegrationConfig(inline, applicationRoot() ?? viteConfig.root, {
      target: inheritCssTarget(cssTarget),
    });
    for (const state of environments.values()) {
      state.close();
    }
    environments.clear();
    bindings = new WeakMap();
    for (const key of Object.keys(config) as (keyof IntegrationConfig)[]) {
      delete config[key];
    }
    Object.assign(config, resolved.config, { emission });
    configurationFiles.clear();
    configurationDependencies = resolved.dependencies;
    missingConfigurations = resolved.missing;
    for (const file of [...resolved.dependencies, ...resolved.missing]) {
      configurationFiles.add(file);
    }
    resolvedViteConfig = viteConfig;
    configurationDirty = false;
    settings.projectRoot = resolve(viteConfig.root, config.root ?? ".");
    settings.assetsDirectory = viteConfig.build.assetsDir;
    settings.publicBase = viteConfig.base;
    settings.sourceMaps = Boolean(viteConfig.build.sourcemap);
    settings.devSourceMaps = Boolean(viteConfig.css.devSourcemap);
    settings.minifyCss = viteConfig.build.cssMinify !== false;
    settings.cssTargets = settings.minifyCss ? minifyTargets(config.target) : undefined;
    settings.watching = viteConfig.command === "serve";
    settings.compilerMode = settings.watching ? "development" : "release";
  };

  /** Detaches the current server's change listeners and closes its configuration discovery watchers. */
  let stopCssWatching = () => {};

  /**
   * Replaces server watch registrations and retains the server for module invalidation and HMR.
   * @param server - Configured development server. A null watch setting disables these watches.
   * @throws If a missing configuration's parent directory cannot be watched.
   */
  const attachServer = (server: ViteDevServer) => {
    settings.devServerRef = server;
    stopCssWatching();
    settings.cssWatching = server.config.server.watch !== null;
    if (!settings.cssWatching) {
      return;
    }
    /**
     * Restarts on configuration changes or invalidates affected compiler environments.
     * @param event - Watch event. Only `add`, `change`, and `unlink` are handled.
     * @param file - Changed filesystem path, normalized before dependency lookup.
     */
    const changed = (event: string, file: string) => {
      if (event !== "add" && event !== "change" && event !== "unlink") {
        return;
      }
      const path = resolve(file);
      if (configurationFiles.has(path)) {
        void server.restart();
        return;
      }
      for (const state of environments.values()) {
        state.changed(path);
      }
    };
    /** Closes discovery watchers, detaches server listeners, and clears the CSS-watching flag. */
    const stop = () => {
      for (const watcher of discoveryWatchers) {
        watcher.close();
      }
      server.watcher.off("all", changed);
      server.httpServer?.off("close", stop);
      settings.cssWatching = false;
    };
    // Missing files in ancestor directories need nonrecursive watches. Adding them to Vite's
    // recursive watcher can include every sibling project above the application root.
    const discoveryWatchers = [...new Set(missingConfigurations.map(dirname))].map((directory) =>
      watch(directory, (_event, filename) => {
        if (filename && configurationFiles.has(resolve(directory, String(filename)))) {
          void server.restart();
        }
      }),
    );
    server.watcher.on("all", changed);
    server.watcher.add(configurationDependencies);
    server.httpServer?.once("close", stop);
    stopCssWatching = stop;
  };

  /** Detaches server listeners, closes discovery watchers, and closes every retained compiler environment. */
  const close = () => {
    stopCssWatching();
    for (const state of environments.values()) {
      state.close();
    }
  };

  /**
   * Publishes a CSS snapshot through the matching environment's HMR channel when serving.
   * @param name - Vite environment name, or undefined for `client`.
   * @param context - Resolver and watch operations used to refresh the environment's CSS.
   * @returns A promise that resolves after publication, or immediately when no active server exists.
   * @throws If refreshing the environment or generating its CSS snapshot fails.
   */
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

  const project: ViteProject = {
    forConfig: (viteConfig) => {
      let generation = generations.get(viteConfig);
      if (!generation) {
        generation = claimed ? createViteProject(supplied, applicationRoot) : project;
        claimed = true;
        generations.set(viteConfig, generation);
      }
      return generation;
    },
    config,
    configurationFiles,
    configurationChanged: (id) => {
      if (!configurationFiles.has(resolve(id))) {
        return false;
      }
      configurationDirty = true;
      return true;
    },
    refreshConfiguration: async () => {
      if (configurationDirty && resolvedViteConfig) {
        await configure(resolvedViteConfig);
      }
    },
    settings,
    environment,
    closeEnvironment,
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
  return project;
}
