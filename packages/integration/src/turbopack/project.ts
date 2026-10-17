import { createHash } from "node:crypto";
import { isBuiltin } from "node:module";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import type { BuildConfig, SerializableInlineConfig } from "@stillwind/config";
import { resolveIntegrationConfig } from "@stillwind/config/node";
import {
  cleanModuleId,
  CompilationFailure,
  cssEntrypoint,
  fileModuleDescriptor,
  loadCssGraph,
  nodePluginHost,
  warnRejectedNames,
  type CompilerHost,
  type LoadedCssGraph,
} from "@stillwind/host";
import { scanSource } from "@stillwind/native";
import type { LoaderContext } from "webpack";

import { nativeBindingFiles } from "../binding-files.ts";
import { buildCompilerOptions, IntegrationSession } from "../session.ts";

/** Removes readonly constraints recursively so cloned inline settings fit the bundler's loader transport. */
type MutableSettings<T> = T extends readonly (infer Item)[]
  ? MutableSettings<Item>[]
  : T extends object
    ? { -readonly [K in keyof T]: MutableSettings<T[K]> }
    : T;

/** Serializable compiler settings and entry selection passed to Turbopack's isolated loaders. */
export type TurbopackLoaderOptions = {
  /** Configuration file selection and serializable inline settings. */
  compiler: MutableSettings<SerializableInlineConfig>;
  /** Application directory, absolute or relative to the loader context's root. */
  root: string;
  /** Entry files whose imports are traversed when reconstructing document CSS. */
  entries: string[];
  /** Compiler mode, normally `development` or `release`. */
  mode: string;
};

/** Compiler session and host operations owned by one Turbopack or Next loader invocation. */
export interface TurbopackProject {
  /** Evaluated compiler settings with an absolute project root, captured entry selection, and mode. */
  options: Omit<TurbopackLoaderOptions, "compiler"> & {
    /** Evaluated settings selected from configuration and inline overrides. */
    compiler: BuildConfig;
  };
  /** Initialized compiler. The caller must close it after compilation or stylesheet reconstruction. */
  compiler: IntegrationSession;
  /** Filesystem, source metadata, resolver, dependency watcher, and warning operations. */
  host: TurbopackHost;
  /** Completed CSS graph and authored resources used by the compiler. */
  loaded: LoadedCssGraph;
  /**
   * Resolves an asset with the loader context's default resolver.
   *
   * @param specifier - Resource request text from CSS or a producer.
   * @param importer - Absolute identity of the resource owner.
   * @returns A promise for the resolved asset identity. Unresolved requests reject the promise.
   */
  resolveAsset: (specifier: string, importer: string) => Promise<string>;
}

/** Compiler host whose resolver returns an identity or rejects instead of returning null. */
export interface TurbopackHost extends CompilerHost {
  /**
   * Resolves a script dependency or marks a Node builtin or Next boundary marker as external.
   *
   * @param specifier - Script request text, including any resource query.
   * @param importer - Absolute identity of the importing source.
   * @returns A promise for the resolved identity and optional external status. Resolution failures
   * reject the promise.
   */
  resolve: (
    specifier: string,
    importer: string,
  ) => Promise<{
    /** Resolved absolute source identity or external module name. */
    id: string;
    /** Whether the graph must skip loading this module as source. */
    external?: boolean;
  }>;
}

/**
 * Loads the design system and creates a compiler for a Turbopack or Next loader invocation.
 *
 * Configuration is evaluated independently in each loader invocation. Configuration dependencies,
 * missing discovery candidates, compiler files, and CSS dependencies are registered with the host.
 *
 * @param context - Loader context containing serializable discovery settings, the bundler root,
 * filesystem access, resolver operations, and dependency tracking.
 * @returns A promise for the initialized project. The caller owns the compiler's lifetime.
 * @throws If configuration evaluation or project selection fails, the CSS graph cannot be loaded,
 * or compiler initialization fails.
 */
export async function createProject(
  context: LoaderContext<TurbopackLoaderOptions>,
): Promise<TurbopackProject> {
  const supplied = context.getOptions();
  const configuration = await resolveIntegrationConfig(
    supplied.compiler,
    path.resolve(context.rootContext, supplied.root),
  );
  const options = {
    ...supplied,
    compiler: configuration.config,
    root: configuration.config.root,
  };
  /**
   * Registers a file whose changes invalidate this loader result.
   * @param id - Filesystem path for configuration, compiler code, CSS, or a resolved dependency.
   */
  const watch = (id: string) => {
    context.addDependency(id);
  };
  for (const file of configuration.dependencies) {
    watch(file);
  }
  for (const file of configuration.missing) {
    context.addMissingDependency(file);
  }
  for (const file of nativeBindingFiles()) {
    watch(file);
  }
  watch(fileURLToPath(import.meta.resolve("@stillwind/host")));
  watch(import.meta.filename);
  for (const file of ["loader.js", "css-loader.js"]) {
    watch(
      fileURLToPath(new URL(`./${file}`, import.meta.resolve("@stillwind/integration/turbopack"))),
    );
  }
  /**
   * Reads text through Turbopack's loader filesystem.
   * @param id - File path accepted by the loader filesystem.
   * @returns File contents converted to a string.
   * @throws If the filesystem reports a read error.
   */
  const read = (id: string) =>
    new Promise<string>((done, reject) => {
      context.fs.readFile(id, (error, contents) =>
        error ? reject(error) : done(String(contents)),
      );
    });
  /**
   * Adapts a Turbopack resolver to imports relative to a source file.
   * @param settings - Resolution policy passed to the loader's resolver factory.
   * @returns An asynchronous resolver accepting a specifier and full importer identity. Its promise
   * rejects on resolver errors or missing results, and otherwise returns the resolved identity.
   */
  const resolver = (settings: Parameters<typeof context.getResolve>[0]) => {
    const resolve = context.getResolve(settings);
    return (specifier: string, importer: string) =>
      new Promise<string>((done, reject) => {
        resolve(path.dirname(cleanModuleId(importer)), specifier, (error, id) =>
          error
            ? reject(error)
            : id
              ? done(id)
              : reject(new Error(`Cannot resolve ${specifier} from ${importer}`)),
        );
      });
  };
  const resolveJs = resolver({
    extensions: [".tsx", ".ts", ".jsx", ".js", ".mjs", ".cjs", ".json"],
    dependencyType: "esm",
  });
  const resolveAsset = resolver({});
  const loaded = await loadCssGraph(
    path.resolve(options.root, cssEntrypoint(options.compiler)),
    buildCompilerOptions(options.compiler),
    {
      read,
      documentUri: (id) => pathToFileURL(id).href,
      sourceMapSource: (id) => pathToFileURL(id).href,
      watch,
      resolve: resolver({
        conditionNames: ["style", "import", "default"],
        extensions: [".css"],
        mainFields: ["style", "main"],
      }),
      plugins: nodePluginHost,
    },
    undefined,
    undefined,
    {
      plugins: options.compiler.plugins,
      root: options.root,
    },
  );
  const compiler = new IntegrationSession(
    options.root,
    loaded.loader,
    options.compiler,
    false,
    options.mode,
    loaded.plugins,
  );
  const host = {
    describe: (id: string) => fileModuleDescriptor(options.root, id),
    read,
    watch,
    resolve: async (specifier: string, importer: string) =>
      // Next checks boundary markers when it builds server and client modules.
      // They provide no styles for this graph and do not use its loader resolver.
      isBuiltin(specifier) || specifier === "server-only" || specifier === "client-only"
        ? {
            id: specifier,
            external: true,
          }
        : {
            id: await resolveJs(specifier, importer),
          },
    warn: (message: string) => {
      context.emitWarning(new Error(message));
    },
  };
  return {
    options,
    compiler,
    host,
    loaded,
    resolveAsset,
  };
}

/**
 * Compiles entry-reachable local JS/TS producers and requests one document stylesheet plan.
 *
 * @param entries - Entry paths, absolute or relative to the evaluated project root. Statically
 * named imports, including lazy imports, are followed. Dynamic request expressions are not traversed.
 * @param project - Initialized loader project. The caller closes its compiler after this operation.
 * @returns A document plan selecting automatic producers, with a revision derived from visited
 * source paths and contents. Explicit CSS values remain owned by their modules.
 * @throws If source resolution, reading, scanning, compilation, or stylesheet planning fails.
 */
export async function compileGraph(entries: readonly string[], project: TurbopackProject) {
  const { options, compiler, host } = project;
  const pending = entries.map((entry) => path.resolve(options.root, entry));
  const visited = new Set<string>();
  const selection: string[] = [];
  const revision = createHash("sha256");
  while (pending.length) {
    const id = pending.shift()!;
    if (visited.has(id)) {
      continue;
    }
    visited.add(id);
    host.watch(id);
    const source = await host.read(id);
    revision.update(id).update("\0").update(source).update("\0");
    const descriptor = fileModuleDescriptor(options.root, id);
    const scan = scanSource({
      kind: "direct",
      moduleId: descriptor.moduleId,
      document: {
        ...descriptor.document,
        code: source,
      },
      syntax: descriptor.syntax,
    });
    if (scan.kind === "failed") {
      throw new CompilationFailure([scan.batch]);
    }
    const summary = scan.summary;
    for (const { source: specifier } of summary.moduleSpecifiers) {
      if (isBuiltin(specifier) || specifier === "stillwind" || specifier.startsWith("stillwind/")) {
        continue;
      }
      const resolved = await host.resolve(specifier, id);
      const file = cleanModuleId(resolved.id);
      if (
        !resolved.external &&
        !/[\\/]node_modules[\\/]/.test(file) &&
        /\.[cm]?[jt]sx?$/.test(file)
      ) {
        pending.push(file);
      }
    }
    if (!summary.macroImports && !summary.moduleCssImports) {
      continue;
    }
    const output = await compiler.compile(source, id, host, {
      moduleCssImport: "stillwind/module-css",
    });
    if (output && !output.hasModuleCssImport) {
      selection.push(output.moduleId);
    }
  }
  const plan = compiler.native.planStylesheet({
    modules: selection,
    coverage: {
      kind: "complete",
      target: "entry-graph",
      revision: revision.digest("hex"),
    },
    placement: "document",
  });
  warnRejectedNames(plan.rejectedNames, (message) => host.warn(message));
  return plan;
}
