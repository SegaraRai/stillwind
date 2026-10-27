import process from "node:process";

import type { InlineConfig } from "@stillwind/config";
import { resolveIntegrationConfig, type ResolvedIntegrationConfig } from "@stillwind/config/node";

import { installEmission, type EmissionCompilation, type EmissionModule } from "./emission.ts";
import type { ResolveCallback, ResolverHost, ResolveSettings } from "./host.ts";
import { stillwindRules } from "./rules.ts";

/**
 * Configures producer loaders and reconstructs document CSS for webpack and Rspack compilations.
 */
export class StillwindPlugin {
  /**
   * Retains configuration selection and overrides for the plugin's loader rules.
   * Files are evaluated in beforeRun and watchRun. The resulting settings are shared by that run's
   * compilations. Constructor execution does not evaluate configuration.
   *
   * @param config - Configuration discovery, project selection, and inline compiler overrides.
   * Omission discovers stillwind.config.* from the compiler context or process working directory.
   */
  constructor(protected readonly config: InlineConfig = {}) {}

  /**
   * Prepends Stillwind rules before framework plugins can clone the rule list.
   *
   * The host retains its existing CSS pipeline for document carriers. Register this plugin before
   * `VueLoaderPlugin` so cloned rules include Stillwind.
   *
   * @param rules - Mutable rule list from the compiler configuration.
   */
  protected configureRules(rules: unknown[]): void {
    rules.unshift(...stillwindRules(this.config));
  }

  /**
   * Installs loader rules, configuration evaluation hooks, and CSS reconstruction hooks.
   * Evaluation errors fail the build run. Existing and missing configuration dependencies are
   * registered on each compilation for watch tracking.
   *
   * @param compiler - webpack or Rspack compiler with a usable input filesystem and resolver. Call
   * through the bundler's plugin setup before compilation begins.
   */
  apply<Module extends EmissionModule>(compiler: EmissionCompiler<Module>): void {
    compiler.options.module ??= {};
    compiler.options.module.rules ??= [];
    this.configureRules(compiler.options.module.rules);
    let configuration: ResolvedIntegrationConfig | undefined;
    /**
     * Evaluates current configuration before a normal or watch build run.
     * @returns Completion after the settings shared by subsequent compilations have been replaced.
     * @throws If configuration discovery, evaluation, or project selection fails.
     */
    const configure = async () => {
      configuration = await resolveIntegrationConfig(
        this.config,
        compiler.options.context ?? process.cwd(),
      );
    };
    compiler.hooks.beforeRun.tapPromise("StillwindConfiguration", configure);
    compiler.hooks.watchRun.tapPromise("StillwindConfiguration", configure);
    compiler.hooks.thisCompilation.tap("StillwindEmission", (compilation) => {
      if (!configuration) {
        throw new Error("Stillwind configuration has not been initialized");
      }
      compilation.__stillwindConfiguration = configuration;
      for (const file of configuration.dependencies) {
        compilation.fileDependencies.add(file);
      }
      for (const file of configuration.missing) {
        compilation.missingDependencies.add(file);
      }
      installEmission({
        _compilation: compilation,
        fs: compiler.inputFileSystem!,
        mode: compiler.options.mode,
        /**
         * Adapts the compiler's normal resolver for document CSS reconstruction.
         * @param settings - Requested dependency conditions, extensions, and metadata fields.
         * @returns A callback resolver using the compiler's configured input filesystem.
         */
        getResolve(settings) {
          const resolver = compiler.resolverFactory.get("normal", settings);
          return (directory, specifier, callback) =>
            resolver.resolve({}, directory, specifier, {}, callback);
        },
      });
    });
  }
}

/**
 * Compiler operations shared by webpack and Rspack for installing rules and registering each
 * compilation.
 */
interface EmissionCompiler<Module extends EmissionModule> {
  /** Run and compilation hooks used to evaluate configuration before registering emission. */
  hooks: {
    /** Normal build hook that initializes settings before thisCompilation. */
    beforeRun: {
      /**
       * Registers asynchronous work before a normal build run.
       * @param name - Plugin registration name.
       * @param callback - Configuration initializer whose rejection fails the run.
       */
      tapPromise(name: string, callback: () => Promise<void>): void;
    };
    /** Watch build hook that reevaluates settings before each compilation. */
    watchRun: {
      /**
       * Registers asynchronous work before a watch build run.
       * @param name - Plugin registration name.
       * @param callback - Configuration initializer whose rejection fails the run.
       */
      tapPromise(name: string, callback: () => Promise<void>): void;
    };
    /** Hook that exposes each new compilation after run configuration has been initialized. */
    thisCompilation: {
      /**
       * Registers settings, dependency watches, and emission on a new compilation.
       * @param name - Plugin registration name.
       * @param callback - Operation receiving the current compilation.
       */
      tap(name: string, callback: (compilation: EmissionCompilation<Module>) => void): void;
    };
  };
  /** Bundler input filesystem, which must be initialized before compilation registration. */
  inputFileSystem: ResolverHost["fs"] | null;
  /** Mutable compiler options to which Stillwind loader rules are added. */
  options: {
    /** Application directory for configuration discovery. Omission uses process.cwd(). */
    context?: string;
    /** Bundler mode forwarded to reconstruction. Only development selects development compilation. */
    mode?: string;
    /** Module rule configuration, initialized by apply when absent. */
    module?: {
      /** Mutable rules retained by the compiler and extended before framework plugins clone them. */
      rules?: unknown[];
    };
  };
  /** Resolver factory bound to this compiler's filesystem and configuration. */
  resolverFactory: {
    /**
     * Obtains a bundler resolver with a selected policy.
     * @param type - Resolver category. Stillwind requests normal.
     * @param settings - Import conditions, extensions, and package fields.
     * @returns Resolver that reports completion through its callback.
     */
    get(
      type: string,
      settings: ResolveSettings,
    ): {
      /**
       * Resolves a request without changing compiler state.
       * @param context - Bundler request metadata. Stillwind passes an empty object.
       * @param directory - Absolute importer directory.
       * @param specifier - Authored import or resource request.
       * @param resolveContext - Bundler resolution metadata. Stillwind passes an empty object.
       * @param callback - Receives a failure or resolved identity.
       */
      resolve(
        context: object,
        directory: string,
        specifier: string,
        resolveContext: object,
        callback: ResolveCallback,
      ): void;
    };
  };
}
