import { createHash } from "node:crypto";
import { watch as watchDirectory, type FSWatcher } from "node:fs";
import { readFile } from "node:fs/promises";
import { isBuiltin } from "node:module";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import type { BuildConfig, SerializableInlineConfig } from "@stillwind/config";
import { resolveIntegrationConfig, type ResolvedIntegrationConfig } from "@stillwind/config/node";
import {
  cleanModuleId,
  CompilationFailure,
  cssEntrypoint,
  fileModuleDescriptor,
  loadCssGraph,
  nodePluginHost,
  resolveTypeFactsEnvironment,
  warnRejectedNames,
  type CompilerHost,
  type LoadedCssGraph,
} from "@stillwind/host";
import { scanSource } from "@stillwind/native";
import type { LoaderContext } from "webpack";

import { nativeBindingFiles } from "../binding-files.ts";
import { LoaderCache, recordInputs, type InputReader } from "../loader-cache.ts";
import {
  buildCompilerOptions,
  IntegrationSession,
  SharedTypeFacts,
  warnSkippedCss,
} from "../session.ts";
import { buildSourceEntries } from "../source-entries.ts";

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

/** Compiler session and host operations that one Turbopack or Next loader invocation uses. */
export interface TurbopackProject {
  /** Evaluated compiler settings with an absolute project root, captured entry selection, and mode. */
  options: Omit<TurbopackLoaderOptions, "compiler"> & {
    /** Evaluated settings selected from configuration and inline overrides. */
    compiler: BuildConfig;
  };
  /**
   * Compiler that other invocations in the same process may use at the same time. The caller must
   * not close it.
   */
  compiler: IntegrationSession;
  /**
   * Compiler that only {@link compileGraph} uses, so that its compiled modules match the sources
   * that the previous reconstruction read. Resource owners in its plans resolve through it.
   */
  graphCompiler: IntegrationSession;
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
  /**
   * Ends this invocation's use of the compiler and design system. The caller must call it after
   * compilation or stylesheet reconstruction, and must not use the project afterward.
   */
  release: () => void;
}

/** Design system and compiler shared by loader invocations with equal settings. */
interface SharedDesignSystem {
  loaded: LoadedCssGraph;
  compiler: IntegrationSession;
  graphCompiler: IntegrationSession;
  /**
   * Checker and prepared sources of both compilers. Absent in development when the process cannot
   * watch the application directory.
   */
  shared?: SharedTypeFacts;
  /** Whether the directory watcher missed changes since the compilers last compared their sources. */
  unverified?: boolean;
  /** Comparison of the sources with the files after missed changes, which loader calls await. */
  verification?: Promise<void>;
}

/**
 * Evaluated configuration keyed by loader options and the application directory. It is evaluated
 * again when one of its files changes.
 */
const configurations = new LoaderCache<ResolvedIntegrationConfig>();

/**
 * Design systems keyed by evaluated settings and mode. The checker of an idle compiler stops after
 * this time, so that a worker process does not keep a checker process running.
 */
const designSystems = new LoaderCache<SharedDesignSystem>(closeDesignSystem, 30_000);

/**
 * Closes the checkers of a design system's compilers and discards their shared sources. The design
 * system stays usable, and a later compilation loads the sources again.
 *
 * @param designSystem - Design system whose compilers to close.
 */
function closeDesignSystem({ compiler, graphCompiler, shared }: SharedDesignSystem): void {
  compiler.close();
  graphCompiler.close();
  shared?.close();
}

/**
 * Design systems of development compilers, keyed by the application directory that one recursive
 * watcher of this process observes. A design system that the cache dropped can be collected.
 */
const sourceWatchers = new Map<string, Set<WeakRef<SharedDesignSystem>>>();

/**
 * Starts or reuses a recursive watcher that reports changes of script, component, and JSON files
 * under an application directory to the compilers of registered design systems. Changes under
 * `node_modules`, `.next`, and `.git` are not reported. When the watcher reports a change without
 * a file name, which happens when changes overflow its buffer, the registered design systems
 * compare their sources with the files before their next use. When the watcher fails, it closes and
 * every registered design system discards its shared sources.
 *
 * Events can arrive after a compilation already read a changed file, so callers must also report
 * the text that they read through {@link IntegrationSession.sourceRead}.
 *
 * @param root - Absolute application directory.
 * @returns The set in which to register design systems, or undefined when the platform cannot
 * watch the directory recursively.
 */
function watchSources(root: string): Set<WeakRef<SharedDesignSystem>> | undefined {
  const existing = sourceWatchers.get(root);
  if (existing) {
    return existing;
  }
  const targets = new Set<WeakRef<SharedDesignSystem>>();
  /**
   * Calls an operation for every registered design system that is still reachable.
   * @param operation - Operation that receives the design system.
   */
  const each = (operation: (designSystem: SharedDesignSystem) => void) => {
    for (const target of targets) {
      const designSystem = target.deref();
      if (designSystem) {
        operation(designSystem);
      } else {
        targets.delete(target);
      }
    }
  };
  let watcher: FSWatcher;
  try {
    watcher = watchDirectory(root, { recursive: true, persistent: false }, (event, name) => {
      if (!name) {
        each((designSystem) => {
          designSystem.unverified = true;
        });
        return;
      }
      if (
        /(?:^|[\\/])(?:node_modules|\.next|\.git)[\\/]/.test(name) ||
        !/\.(?:[cm]?[jt]sx?|json|astro|svelte|vue)$/.test(name)
      ) {
        return;
      }
      const file = path.join(root, name);
      // A rename event reports an added or removed file, which can change import resolution.
      each(({ compiler, graphCompiler }) => {
        compiler.sourceChanged(file, event === "rename");
        graphCompiler.sourceChanged(file, event === "rename");
      });
    });
  } catch {
    return undefined;
  }
  watcher.on("error", () => {
    watcher.close();
    sourceWatchers.delete(root);
    each(closeDesignSystem);
  });
  sourceWatchers.set(root, targets);
  return targets;
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
 * Loads the design system and compiler for a Turbopack or Next loader invocation, or reuses them.
 *
 * Invocations in one process share the evaluated configuration while its files keep their contents,
 * and share the design system and compiler while the configuration and the CSS files stay
 * unchanged. Configuration dependencies, missing discovery candidates, compiler files, and CSS
 * dependencies are registered with the loader context of every invocation.
 *
 * @param context - Loader context containing serializable discovery settings, the bundler root,
 * filesystem access, resolver operations, and dependency tracking.
 * @returns A promise for the project. The caller must call its `release` function after its last use.
 * @throws If configuration evaluation or project selection fails, the CSS graph cannot be loaded,
 * or compiler initialization fails.
 */
export async function createProject(
  context: LoaderContext<TurbopackLoaderOptions>,
): Promise<TurbopackProject> {
  const supplied = context.getOptions();
  const applicationRoot = path.resolve(context.rootContext, supplied.root);
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
  const probe: InputReader = (id) =>
    read(id).catch((error: NodeJS.ErrnoException) => {
      if (error.code === "ENOENT" || error.code === "ENOTDIR") {
        return undefined;
      }
      throw error;
    });
  const configurationLease = await configurations.use(
    JSON.stringify([supplied.compiler, applicationRoot]),
    probe,
    async () => {
      const value = await resolveIntegrationConfig(supplied.compiler, applicationRoot);
      const recorder = recordInputs(probe);
      for (const file of value.dependencies) {
        recorder.watch(file);
      }
      for (const file of value.missing) {
        recorder.watchMissing(file);
      }
      return { value, inputs: await recorder.finish() };
    },
  );
  configurationLease.done();
  const configuration = configurationLease.value;
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
  const designSystem = await designSystems.use(
    JSON.stringify([options.compiler, options.root, options.mode]),
    probe,
    async () => {
      const recorder = recordInputs(probe);
      const loaded = await loadCssGraph(
        path.resolve(options.root, cssEntrypoint(options.compiler)),
        buildCompilerOptions(options.compiler),
        {
          read: recorder.read,
          documentUri: (id) => pathToFileURL(id).href,
          sourceMapSource: (id) => pathToFileURL(id).href,
          watch: recorder.watch,
          watchDirectory: recorder.watchDirectory,
          // The resolver belongs to this invocation's context. Later invocations reuse only the
          // loaded graph, which does not resolve CSS again.
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
          tailwind: options.compiler.tailwind,
          important: options.compiler.important,
          root: options.root,
        },
      );
      // Both compilers load the project files into one checker when its project is empty. Files
      // do not change during a production build. A development server does not report file
      // changes to loaders, so the process watches the application directory itself. When it
      // cannot, each compilation reads the files again.
      const release = options.mode === "release";
      const entries = options.entries.map((entry) => path.resolve(options.root, entry));
      const watcher = release ? undefined : watchSources(options.root);
      const shared =
        release || watcher
          ? new SharedTypeFacts(
              resolveTypeFactsEnvironment(options.root, options.compiler.tsconfig),
            )
          : undefined;
      const sourceEntries = shared
        ? await buildSourceEntries(entries, configuration.scope)
        : undefined;
      const session = (domain: string) =>
        new IntegrationSession(
          options.root,
          loaded.loader,
          options.compiler,
          false,
          options.mode,
          loaded.plugins,
          release ? sourceEntries : undefined,
          release ? undefined : sourceEntries,
          shared,
          domain,
        );
      const value = {
        loaded,
        compiler: session("loader"),
        graphCompiler: session("graph"),
        shared,
      };
      watcher?.add(new WeakRef(value));
      return {
        value,
        inputs: await recorder.finish(),
      };
    },
  );
  const { loaded, compiler, graphCompiler } = designSystem.value;
  if (designSystem.value.unverified) {
    designSystem.value.unverified = false;
    // The loader filesystem can return the previous text of a file right after an edit.
    const current = (id: string) =>
      readFile(id, "utf8").catch((error: NodeJS.ErrnoException) => {
        if (error.code === "ENOENT" || error.code === "ENOTDIR") {
          return undefined;
        }
        throw error;
      });
    designSystem.value.verification = Promise.all([
      compiler.verifySources(current),
      graphCompiler.verifySources(current),
    ]).then(() => {});
  }
  try {
    await designSystem.value.verification;
  } catch (error) {
    designSystem.done();
    throw error;
  }
  for (const file of designSystem.inputs.files.keys()) {
    watch(file);
  }
  for (const file of designSystem.inputs.missing) {
    context.addMissingDependency(file);
  }
  for (const directory of designSystem.inputs.directories) {
    context.addContextDependency(directory);
  }
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
            // Turbopack's resolver rewrites an absolute request to a relative one with native
            // separators, which it cannot resolve. Project files reach the compiler as absolute
            // paths.
            id: path.isAbsolute(specifier)
              ? path.normalize(specifier)
              : await resolveJs(specifier, importer),
          },
    warn: (message: string) => {
      context.emitWarning(new Error(message));
    },
  };
  return {
    options,
    compiler,
    graphCompiler,
    host,
    loaded,
    resolveAsset,
    release: designSystem.done,
  };
}

/**
 * Compiles entry-reachable local JS/TS producers and requests one document stylesheet plan.
 *
 * Calls with the same graph compiler run one at a time. A producer is compiled again only when its
 * source or the source of a local module that it reaches changed since the previous call.
 *
 * @param entries - Entry paths, absolute or relative to the evaluated project root. Statically
 * named imports, including lazy imports, are followed. Dynamic request expressions are not traversed.
 * @param project - Loader project. The caller releases it after this operation.
 * Its host receives warnings for the CSS statements that loading skipped and for rejected names.
 * @returns A document plan selecting automatic producers, with a revision derived from visited
 * source paths and contents. Explicit CSS values remain owned by their modules.
 * @throws If source resolution, reading, scanning, compilation, or stylesheet planning fails.
 */
export function compileGraph(entries: readonly string[], project: TurbopackProject) {
  const previous = graphRuns.get(project.graphCompiler) ?? Promise.resolve();
  const run = previous.catch(() => {}).then(() => reconstruct(entries, project));
  graphRuns.set(project.graphCompiler, run);
  return run;
}

/**
 * Compiles the entry graph and plans its stylesheet for {@link compileGraph}.
 *
 * @param entries - Entry paths, absolute or relative to the evaluated project root.
 * @param project - Loader project whose graph compiler no other reconstruction uses meanwhile.
 * @returns A document plan for the automatic producers that the entries reach.
 * @throws If source resolution, reading, scanning, compilation, or stylesheet planning fails.
 */
async function reconstruct(entries: readonly string[], project: TurbopackProject) {
  const { options, graphCompiler: compiler, host } = project;
  const pending = entries.map((entry) => path.resolve(options.root, entry));
  const visited = new Set<string>();
  const revision = createHash("sha256");
  const nodes: GraphNode[] = [];
  while (pending.length) {
    const id = pending.shift()!;
    if (visited.has(id)) {
      continue;
    }
    visited.add(id);
    host.watch(id);
    const source = await host.read(id);
    compiler.sourceRead(id, source);
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
    const imports: string[] = [];
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
        imports.push(file);
        pending.push(file);
      }
    }
    nodes.push({
      id,
      source,
      imports,
      producer: Boolean(summary.macroImports || summary.moduleCssImports),
    });
  }
  const outputs = graphOutputs.get(compiler) ?? new Map<string, GraphOutput>();
  graphOutputs.set(compiler, outputs);
  // A module's output depends on the types of the local modules that it imports, so a change
  // also compiles every module that reaches the changed module.
  const importers = new Map<string, string[]>();
  for (const node of nodes) {
    for (const imported of node.imports) {
      importers.set(imported, [...(importers.get(imported) ?? []), node.id]);
    }
  }
  const byId = new Map(nodes.map((node) => [node.id, node]));
  const stale = new Set<string>();
  const queue = nodes.filter((node) => outputs.get(node.id)?.source !== node.source);
  for (let node = queue.pop(); node; node = queue.pop()) {
    if (stale.has(node.id)) {
      continue;
    }
    stale.add(node.id);
    for (const importer of importers.get(node.id) ?? []) {
      queue.push(byId.get(importer)!);
    }
  }
  const selection: string[] = [];
  for (const node of nodes) {
    let output = outputs.get(node.id);
    if (stale.has(node.id) || !output) {
      const compiled = node.producer
        ? await compiler.compile(node.source, node.id, host, {
            moduleCssImport: "stillwind/module-css",
          })
        : undefined;
      output = {
        source: node.source,
        selected: compiled && !compiled.hasModuleCssImport ? compiled.moduleId : undefined,
      };
      outputs.set(node.id, output);
    }
    if (output.selected) {
      selection.push(output.selected);
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
  warnSkippedCss(project.loaded.skipped, (message) => host.warn(message));
  warnRejectedNames(plan.rejectedNames, (message) => host.warn(message));
  return plan;
}

/** Local JavaScript or TypeScript module that a graph traversal visited. */
interface GraphNode {
  /** Absolute file path. */
  id: string;
  source: string;
  /** Absolute paths of the local modules that the source imports. */
  imports: string[];
  /** Whether the source imports a Stillwind macro or module CSS, so that it needs compilation. */
  producer: boolean;
}

/** Result of the last graph compilation of one module. */
interface GraphOutput {
  /** Source text that the result was compiled from. */
  source: string;
  /** Compiler module ID that the document plan selects, or undefined when it selects nothing. */
  selected?: string;
}

/**
 * Last graph compilation results of each graph compiler, keyed by absolute file path. A compiler
 * keeps the compiled state of every module, so a reconstruction compiles only modules whose sources
 * or imported modules changed since the previous reconstruction. Only reconstructions compile with
 * a graph compiler: a per-module compilation of another source would replace a module's state
 * without changing its result here.
 */
const graphOutputs = new WeakMap<IntegrationSession, Map<string, GraphOutput>>();

/**
 * Latest reconstruction of each graph compiler. A concurrent reconstruction could publish a module
 * from an older snapshot after a newer reconstruction recorded the newer source.
 */
const graphRuns = new WeakMap<IntegrationSession, Promise<unknown>>();
