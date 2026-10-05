import type { StillwindOptions } from "../options.ts";
import { installEmission, type EmissionCompilation, type EmissionModule } from "./emission.ts";
import type { ResolverHost, ResolveSettings, ResolveCallback } from "./host.ts";
import { stillwindRules } from "./rules.ts";

/**
 * Configures producer loaders and reconstructs document CSS for webpack and Rspack compilations.
 */
export class StillwindPlugin {
  /**
   * Creates a plugin with the compiler configuration shared by its loader rules.
   *
   * @param options - Configuration containing a `css` or `theme` entrypoint. Relative entrypoint
   * paths use the project root selected by applying `options.root` to the bundler root.
   * @throws If neither CSS entrypoint is supplied.
   */
  constructor(protected readonly options: StillwindOptions) {
    if (!options.css && !options.theme) {
      throw new Error("Stillwind requires a CSS entrypoint");
    }
  }

  /**
   * Prepends Stillwind rules before framework plugins can clone the rule list.
   *
   * The host retains its existing CSS pipeline for document carriers. Register this plugin before
   * `VueLoaderPlugin` so cloned rules include Stillwind.
   *
   * @param rules - Mutable rule list from the compiler configuration.
   */
  protected configureRules(rules: unknown[]): void {
    rules.unshift(...stillwindRules(this.options));
  }

  /**
   * Installs loader rules and registers CSS reconstruction for each new compilation.
   *
   * @param compiler - webpack or Rspack compiler with a usable input filesystem and resolver. Call
   * through the bundler's plugin setup before compilation begins.
   */
  apply<Module extends EmissionModule>(compiler: EmissionCompiler<Module>): void {
    compiler.options.module ??= {};
    compiler.options.module.rules ??= [];
    this.configureRules(compiler.options.module.rules);
    compiler.hooks.thisCompilation.tap("StillwindEmission", (compilation) => {
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
    thisCompilation: {
      tap(name: string, callback: (compilation: EmissionCompilation<Module>) => void): void;
    };
  };
  inputFileSystem: ResolverHost["fs"] | null;
  options: {
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
