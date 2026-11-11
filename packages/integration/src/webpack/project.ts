/**
 * Shares one loaded design system and compiler session between the producer loader calls and the
 * document CSS reconstruction of a webpack or Rspack compiler.
 *
 * The main entry point is {@link useProject}.
 */
import path from "node:path";
import { pathToFileURL } from "node:url";

import type { BuildConfig } from "@stillwind/config";
import type { ProjectScope } from "@stillwind/config/node";
import { cssEntrypoint, loadCssGraph, nodePluginHost, type LoadedCssGraph } from "@stillwind/host";

import { LoaderCache, recordInputs, type InputReader } from "../loader-cache.ts";
import { buildCompilerOptions, IntegrationSession } from "../session.ts";
import { buildSourceEntries, isSourceEntry } from "../source-entries.ts";

/**
 * Compiler operations of webpack and Rspack that a shared project uses.
 */
export interface ProjectCompiler {
  /** Top compiler of a child compiler. Child compilers use the projects of their top compiler. */
  root?: ProjectCompiler;
  /** Application directory against which relative entry requests resolve. */
  context?: string;
  /** Whether the compiler runs in watch mode, so that later compilations follow file changes. */
  watchMode?: boolean;
  /** Absolute paths that changed or were added since the previous compilation of a watch run. */
  modifiedFiles?: ReadonlySet<string>;
  /** Absolute paths that were removed since the previous compilation of a watch run. */
  removedFiles?: ReadonlySet<string>;
  /** Normalized entry settings. A function selects entries at compilation time. */
  options?: {
    /** Entry descriptions keyed by name, or a function. */
    entry?: unknown;
  };
  /** Hooks that end the compiler's use of its shared projects. */
  hooks: {
    /** Runs after each compilation finishes. */
    done: {
      /**
       * @param name - Plugin registration name.
       * @param callback - Operation run after the compilation.
       */
      tap(name: string, callback: () => void): void;
    };
    /** Runs when the compiler closes. Absent in compilers without a shutdown hook. */
    shutdown?: {
      /**
       * @param name - Plugin registration name.
       * @param callback - Operation run when the compiler closes.
       */
      tap(name: string, callback: () => void): void;
    };
  };
}

/**
 * Loader context operations that {@link useProject} uses.
 */
export interface ProjectContext {
  /** Compiler that runs the loader, or undefined outside a webpack or Rspack compilation. */
  _compiler?: ProjectCompiler;
  /** Compilation that runs the loader, used to apply each compilation's file changes once. */
  _compilation?: object;
  /**
   * Registers a file whose changes invalidate this loader result.
   *
   * @param id - Absolute file path.
   */
  addDependency(id: string): void;
  /**
   * Registers a missing file whose creation invalidates this loader result.
   *
   * @param id - Absolute file path.
   */
  addMissingDependency(id: string): void;
  /**
   * Registers a directory whose listing affects this loader result.
   *
   * @param id - Absolute directory path.
   */
  addContextDependency(id: string): void;
}

/** File operations of one loader call that loading the CSS graph uses. */
export interface ProjectHost {
  /**
   * Reads a file through the bundler filesystem.
   *
   * @param id - Absolute file path.
   * @returns A promise for the file text, rejecting when the file cannot be read.
   */
  read: (id: string) => Promise<string>;
  /**
   * Resolves a CSS import.
   *
   * @param specifier - CSS import text.
   * @param importer - Absolute path of the importing stylesheet.
   * @returns A promise for the resolved path, or an empty string when the resolver declines it.
   */
  resolveCss: (specifier: string, importer: string) => Promise<string>;
}

/**
 * Design system and compiler that loader calls with equal settings share.
 */
export interface SharedProject {
  /** Completed CSS graph and authored resources used by the compiler. */
  loaded: LoadedCssGraph;
  /** Compiler that concurrent loader calls use. The caller must not close it. */
  compiler: IntegrationSession;
  /** Compilations whose file changes the compiler has received. */
  applied: WeakSet<object>;
  /**
   * Last loader compilation of each producer, keyed by full resolver identity. Document CSS
   * reconstruction compiles a producer again only when its source differs.
   */
  outputs: Map<string, ProducerOutput>;
}

/**
 * Result of a producer loader compilation that the compiler session still holds.
 */
export interface ProducerOutput {
  /** Source text that the compilation received. */
  source: string;
  /** Compiler module ID, or undefined when the compilation produced no code. */
  moduleId?: string;
}

/**
 * Shared projects of each compiler, keyed by settings. A compiler's projects never serve another
 * compiler, because two compilers can resolve the same import differently.
 */
const projects = new WeakMap<object, LoaderCache<SharedProject>>();

/** Shared projects of loader calls that run outside a compiler. */
const detachedProjects = new LoaderCache<SharedProject>(closeProject, 30_000);

/**
 * Closes the checker of a shared project and discards its prepared sources. The project stays
 * usable, and a later compilation starts a new checker.
 *
 * @param project - Project whose checker to close.
 */
function closeProject(project: SharedProject): void {
  project.compiler.close();
}

/**
 * Returns the project cache of a compiler, creating it on first use.
 *
 * A build closes idle checkers after each compilation, because no later compilation of the run
 * uses them. A watch run keeps them until the compiler closes or they stay idle for 30 seconds.
 *
 * @param compiler - Compiler that runs the loader.
 * @returns The compiler's cache.
 */
function compilerProjects(compiler: ProjectCompiler): LoaderCache<SharedProject> {
  let cache = projects.get(compiler);
  if (!cache) {
    const created = new LoaderCache<SharedProject>(closeProject, 30_000);
    cache = created;
    projects.set(compiler, created);
    compiler.hooks.done.tap("StillwindProject", () => {
      if (!compiler.watchMode) {
        created.closeIdle();
      }
    });
    compiler.hooks.shutdown?.tap("StillwindProject", () => created.closeIdle());
  }
  return cache;
}

/**
 * Lists the entry files of a compiler that are local script or component files.
 *
 * @param compiler - Compiler whose normalized entry settings to read.
 * @returns Absolute paths of the relative and absolute entry requests. Package requests and
 * entries that a function selects are not listed.
 */
function compilerEntries(compiler: ProjectCompiler): string[] {
  const entry = compiler.options?.entry;
  if (!entry || typeof entry !== "object") {
    return [];
  }
  const context = compiler.context ?? process.cwd();
  return Object.values(entry).flatMap((description: unknown) => {
    const requests = (description as { import?: unknown } | null)?.import;
    return (Array.isArray(requests) ? requests : []).flatMap((request: unknown) =>
      typeof request === "string" &&
      (request.startsWith(".") || path.isAbsolute(request)) &&
      isSourceEntry(request)
        ? [path.resolve(context, request)]
        : [],
    );
  });
}

/**
 * Loads the design system and compiler for a producer loader call, or reuses them.
 *
 * Calls of one compiler share them while the configuration and the CSS files stay unchanged. In a
 * build, the compiler loads the source files of the entries and of the selected project into one
 * checker before its first compilation. In a watch run, each compilation reports the files that
 * webpack or Rspack saw change to the compiler first. CSS dependencies are registered with the
 * loader context of every call.
 *
 * @param context - Loader context that supplies the compiler and dependency registration.
 * @param scope - Target globs of the selected project, or undefined when no configuration file
 * selects a project.
 * @param config - Evaluated compiler settings.
 * @param root - Absolute project root.
 * @param mode - Native compilation mode.
 * @param host - File reads and CSS resolution of this call. Later calls reuse only the loaded
 * graph, which does not read or resolve CSS again.
 * @returns A lease for the shared project. The caller must end it after its last use.
 * @throws If reading the recorded inputs fails, the CSS graph cannot be loaded, or compiler
 * initialization fails.
 */
export async function useProject(
  context: ProjectContext,
  scope: ProjectScope | undefined,
  config: BuildConfig,
  root: string,
  mode: string,
  host: ProjectHost,
) {
  const compiler = context._compiler?.root ?? context._compiler;
  const cache = compiler ? compilerProjects(compiler) : detachedProjects;
  const probe: InputReader = (id) =>
    host.read(id).catch((error: NodeJS.ErrnoException) => {
      if (error.code === "ENOENT" || error.code === "ENOTDIR") {
        return undefined;
      }
      throw error;
    });
  const lease = await cache.use(JSON.stringify([config, root, mode]), probe, async () => {
    const recorder = recordInputs(probe);
    const loaded = await loadCssGraph(
      path.resolve(root, cssEntrypoint(config)),
      buildCompilerOptions(config),
      {
        resolve: host.resolveCss,
        read: recorder.read,
        documentUri: (id) => pathToFileURL(id).href,
        sourceMapSource: (id) => pathToFileURL(id).href,
        watch: recorder.watch,
        watchDirectory: recorder.watchDirectory,
        plugins: nodePluginHost,
      },
      undefined,
      undefined,
      {
        plugins: config.plugins,
        tailwind: config.tailwind,
        important: config.important,
        root,
      },
    );
    // Without a compiler, no report says which files change, so each compilation reads the files
    // again.
    const entries = compiler
      ? await buildSourceEntries(compilerEntries(compiler), scope)
      : undefined;
    const watching = Boolean(compiler?.watchMode);
    const value: SharedProject = {
      loaded,
      compiler: new IntegrationSession(
        root,
        loaded.loader,
        config,
        false,
        mode,
        loaded.plugins,
        watching ? undefined : entries,
        watching ? entries : undefined,
      ),
      applied: new WeakSet(),
      outputs: new Map(),
    };
    return { value, inputs: await recorder.finish() };
  });
  const { applied, compiler: session } = lease.value;
  const compilation = context._compilation;
  if (compiler?.watchMode && compilation && !applied.has(compilation)) {
    applied.add(compilation);
    for (const file of compiler.removedFiles ?? []) {
      session.sourceChanged(file, true);
    }
    for (const file of compiler.modifiedFiles ?? []) {
      if (!/\.(?:[cm]?[jt]sx?|json|astro|svelte|vue)$/.test(file)) {
        continue;
      }
      // The compiler reports added files as modified. An added file can change how an import
      // resolves, so a file that no prepared source holds counts as added.
      session.sourceChanged(file, !session.holdsSource(file));
    }
  }
  for (const file of lease.inputs.files.keys()) {
    context.addDependency(file);
  }
  for (const file of lease.inputs.missing) {
    context.addMissingDependency(file);
  }
  for (const directory of lease.inputs.directories) {
    context.addContextDependency(directory);
  }
  return lease;
}
