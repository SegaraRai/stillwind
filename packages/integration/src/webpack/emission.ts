import type { CssUpstreamMap } from "@stillwind/host";

import type { StillwindOptions } from "../options.ts";
import { compilationStyles } from "./compilation-styles.ts";
import { createCompilationHost, type ResolverHost } from "./host.ts";

/** A producer and its captured dependencies stored in bundler metadata for CSS reconstruction. */
export interface ProducerInput {
  /** Source text received by the producer loader, before Stillwind compilation. */
  source: string;
  /** Producer identity. Query-qualified script dependencies retain their query suffix. */
  id: string;
  /** Absolute project root against which compiler module identities are resolved. */
  root: string;
  /** Compiler configuration shared by every producer in this compilation. */
  options: StillwindOptions;
  /**
   * Whether this producer is excluded from automatic document CSS and supplies explicit CSS values.
   */
  manual: boolean;
  /** Map from loader input back to the original source, used when composing final CSS maps. */
  upstream?: CssUpstreamMap;
  /**
   * Transformed dependencies captured through the loader chain. Cached metadata retains them for
   * reconstruction.
   */
  dependencies?: {
    /** Complete resolver identity, including the query that selected the transformed dependency. */
    id: string;
    /** Source produced by the loaders that ran before Stillwind. */
    source: string;
    /**
     * Map from the captured source back to its original source, when the preceding loaders supplied
     * one.
     */
    upstream?: CssUpstreamMap;
  }[];
}

/**
 * Stores a producer in module build metadata so cached modules can participate in CSS
 * reconstruction.
 *
 * @param context - Loader context for a module in a compilation registered by `StillwindPlugin`.
 * @param input - Producer source, compiler configuration, and captured dependency sources.
 * @throws If the compilation has no registered emission hook or the loader context has no module.
 */
export function recordProducer<Module extends EmissionModule>(
  context: EmissionContext<Module>,
  input: ProducerInput,
): void {
  requireEmission(context);
  if (!context._module) {
    throw new Error(incompleteGraphMessage);
  }
  (context._module.buildInfo ??= {}).stillwind = input;
}

/**
 * Bundler module metadata used to recover cached producer inputs and identify the shared document
 * CSS carrier.
 */
export interface EmissionModule {
  /** Resource path and query used to identify the shared document CSS carrier. */
  resource?: string;
  /** Build metadata retained by the bundler when a module is reused from cache. */
  buildInfo?: {
    /** Producer input reconstructed after the compilation graph is complete. */
    stillwind?: ProducerInput;
    /** Build fields owned by the bundler or other plugins, retained without interpretation. */
    [key: string]: unknown;
  };
}

/** Compilation operations required to collect producers and rebuild document CSS carriers. */
export interface EmissionCompilation<Module extends EmissionModule = EmissionModule> {
  hooks: {
    finishModules: {
      tapPromise(name: string, callback: (modules: Iterable<Module>) => Promise<void>): void;
    };
  };
  fileDependencies: {
    add(id: string): unknown;
  };
  warnings: {
    push(error: Error): unknown;
  };
  rebuildModule(module: Module, callback: (error?: Error | null) => void): void;
  /**
   * Reconstructed document CSS, optionally including an inline source map. Undefined before
   * reconstruction.
   */
  __stillwindCss?: string;
}

/**
 * Bundler context used to register emission and attach producer metadata to the current
 * compilation.
 */
export interface EmissionContext<
  Module extends EmissionModule = EmissionModule,
> extends ResolverHost {
  _compilation?: EmissionCompilation<Module>;
  _module?: Module;
  /**
   * Bundler mode forwarded to CSS reconstruction. Only `development` selects development
   * compilation.
   */
  mode?: string;
}

/**
 * Registers CSS reconstruction after all modules in the compilation have finished.
 *
 * Registration is idempotent for a compilation. The hook collects cached and newly built producers,
 * stores document CSS, and rebuilds each carrier through the host CSS pipeline.
 *
 * @param context - Context with a compilation, filesystem, and resolver. Register it before
 * producer loaders run.
 * @throws If the context has no compilation with a `finishModules` hook.
 */
export function installEmission<Module extends EmissionModule>(
  context: EmissionContext<Module>,
): void {
  const compilation = requireCompilation(context);
  if (installed.has(compilation)) {
    return;
  }
  installed.add(compilation);
  const io = createCompilationHost(context);
  compilation.hooks.finishModules.tapPromise("StillwindEmission", async (modules) => {
    const completed = [...modules];
    const producers = completed
      .map((module) => module.buildInfo?.stillwind)
      .filter((input): input is ProducerInput => Boolean(input));
    const carriers = completed.filter((module) =>
      module.resource?.includes("stillwind-webpack.css?stillwind-document"),
    );
    if (!carriers.length || !producers.length) {
      return;
    }
    compilation.__stillwindCss = await compilationStyles(
      producers,
      io,
      context.mode,
      (id) => compilation.fileDependencies.add(id),
      (message) => compilation.warnings.push(new Error(message)),
    );
    for (const carrier of carriers) {
      await new Promise<void>((resolve, reject) =>
        compilation.rebuildModule(carrier, (error) => (error ? reject(error) : resolve())),
      );
    }
  });
}

/**
 * Compilations whose emission hooks have been registered. Membership is required before automatic
 * loaders record producers.
 */
const installed = new WeakSet<object>();

const incompleteGraphMessage =
  "Automatic Stillwind CSS requires new StillwindPlugin(options) before compilation begins";

/**
 * Obtains the compilation operations needed to reconstruct document CSS.
 *
 * @param context - Bundler context that must expose a `finishModules` compilation hook.
 * @returns The compilation attached to the context.
 * @throws If the context cannot supply a completed compilation graph.
 */
function requireCompilation<Module extends EmissionModule>(
  context: EmissionContext<Module>,
): EmissionCompilation<Module> {
  const compilation = context._compilation;
  if (!compilation?.hooks?.finishModules) {
    throw new Error(incompleteGraphMessage);
  }
  return compilation;
}

/**
 * Rejects automatic CSS requests when the compilation has no registered emission hook.
 *
 * @param context - Loader context associated with a compilation registered by `StillwindPlugin`.
 * @returns The compilation holding reconstructed document CSS.
 * @throws If the compilation is missing or its emission hook has not been registered.
 */
export function requireEmission<Module extends EmissionModule>(
  context: EmissionContext<Module>,
): EmissionCompilation<Module> {
  const compilation = requireCompilation(context);
  if (!installed.has(compilation)) {
    throw new Error(incompleteGraphMessage);
  }
  return compilation;
}
