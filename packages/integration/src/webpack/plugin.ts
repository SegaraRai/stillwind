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
   * Omission discovers stillwind.config.ts from the compiler context or process working directory.
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
  hooks: {
    beforeRun: {
      tapPromise(name: string, callback: () => Promise<void>): void;
    };
    watchRun: {
      tapPromise(name: string, callback: () => Promise<void>): void;
    };
    thisCompilation: {
      tap(name: string, callback: (compilation: EmissionCompilation<Module>) => void): void;
    };
  };
  inputFileSystem: ResolverHost["fs"] | null;
  options: {
    context?: string;
    mode?: string;
    module?: {
      rules?: unknown[];
    };
  };
  resolverFactory: {
    get(
      type: string,
      settings: ResolveSettings,
    ): {
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
