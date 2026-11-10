import type { BuildConfig } from "@stillwind/config";
import {
  attachPlugins,
  cleanModuleId,
  CompilationProject,
  compileModule,
  compilerOptions,
  ResolutionMismatchError,
  resolveTypeFactsEnvironment,
  TypeScriptFactsService,
  type CompilerHost,
  type ModuleCompileOptions,
  type PluginRuntime,
  type TypeFactsEnvironment,
} from "@stillwind/host";
import {
  NativeProjectSession,
  type NativeCompilerOptions,
  type NativeDesignSystemLoader,
  type NativeSkippedCss,
} from "@stillwind/native";

/**
 * Selects native settings for build and development sessions with lint disabled.
 * @param config - Shared project settings. Analysis hosts retain its lint configuration.
 * @returns Compiler settings for generating JavaScript and CSS.
 */
export function buildCompilerOptions(config: BuildConfig): NativeCompilerOptions {
  return compilerOptions({
    ...config,
    lint: false,
  });
}

/**
 * Reports each statement that loading skipped through a bundler warning channel. Build and
 * development sessions run without stylesheet lint, so these warnings report the skipped statements.
 * @param skipped - Skipped statements of a loaded graph.
 * @param warn - Receives the lint rule, reason, and one-based authored location of each statement.
 */
export function warnSkippedCss(
  skipped: readonly NativeSkippedCss[],
  warn: (message: string) => void,
): void {
  for (const statement of skipped) {
    warn(
      `stillwind: ${statement.rule}: ${statement.message} (${statement.sourceName}:${statement.line + 1}:${statement.column + 1})`,
    );
  }
}

/**
 * Prepared sources, scans, facts, and one checker that several adapter environments of one project
 * share. Each environment resolves imports in its own resolver domain, and the shared project
 * checks that the domains agree. Close it when the adapter releases the project.
 */
export class SharedTypeFacts {
  /** Sources, checker inputs, and facts for every environment that agrees on import resolution. */
  readonly project = new CompilationProject(true);
  /** Lazily started checker. */
  #facts?: TypeScriptFactsService;

  /**
   * @param environment - TypeScript settings that every sharing environment uses.
   */
  constructor(readonly environment: TypeFactsEnvironment) {}

  /** Returns the shared checker, starting it on first use. */
  facts(): TypeScriptFactsService {
    this.#facts ??= new TypeScriptFactsService(this.environment);
    return this.#facts;
  }

  /**
   * Discards shared sources read from a file and every collected fact.
   *
   * @param file - Absolute path of a file that changed, was added, or was removed.
   * @param entry - Whether the file was added or removed rather than changed.
   */
  sourceChanged(file: string, entry: boolean): void {
    this.project.invalidateFile(file, entry);
  }

  /** Closes the checker and discards shared sources. A later compilation starts a new checker. */
  close(): void {
    this.project.invalidate();
    this.#facts?.close();
    this.#facts = undefined;
  }
}

/**
 * Owns a compiler session and a reusable checker for one adapter environment. TypeScript settings
 * are selected at construction. Close the checker when the adapter releases or reloads the project.
 */
export class IntegrationSession {
  /** Native compiler state retained across module updates. Closing this wrapper only releases its checker. */
  readonly native: NativeProjectSession;
  /** Absolute project directory captured when TypeScript settings were selected. */
  readonly root: string;

  /** Fixed TypeScript settings selected before any lazy checker startup. */
  readonly #typeFactsEnvironment: TypeFactsEnvironment;
  /** Compiler-to-resolver correspondences recorded only after compilation returns successfully. */
  readonly #resolverIdsByModuleId = new Map<string, string>();
  /** Reverse correspondences used to reject a host identity that changes compiler identity. */
  readonly #moduleIdsByResolverId = new Map<string, string>();
  /** Lazily started Node checker, released and recreated independently of native compiler state. */
  #facts?: TypeScriptFactsService;
  /**
   * Sources, scans, checker inputs, and facts shared by compilations while source files stay
   * unchanged. Absent when files can change without {@link sourceChanged} being called.
   */
  #project?: CompilationProject;
  /**
   * State shared with other environments, used until an import resolves differently here. The
   * shared checker then serves only the other environments.
   */
  #shared?: SharedTypeFacts;
  /** Resolver domain of this environment in the shared project. */
  readonly #domain?: string;
  /** Entries that a compilation prepares together with its own entry when the project is empty. */
  readonly #entries: readonly string[];
  /** Loading of the entries, which every compilation awaits before it queries the checker. */
  #preparation?: Promise<void>;
  /** Whether {@link #preparation} has not settled. */
  #preparing = false;

  /**
   * Creates compiler state without starting the TypeScript checker.
   *
   * @param root - Absolute adapter project root used for module identities and TypeScript settings.
   * @param loader - Completed design-system loader retained by the native session.
   * @param config - Compiler settings, including the root-relative or absolute tsconfig selection.
   * @param watch - Whether failed native updates retain the last successful output.
   * @param mode - Native compilation mode selected by the adapter.
   * @param plugins - Plugin runtime of the loader's CSS graph, which resolves dynamic utilities.
   * @param fixedEntries - Present when no source file changes before the session closes. The
   * session then scans each file once and gives the checker one project for every compilation.
   * A compilation that finds the project empty prepares the files that these absolute paths reach,
   * together with its own entry, so later compilations seldom add files to the checker project.
   * @param watchedEntries - Present when the caller reports every source file change through
   * {@link sourceChanged}. Compilations then share sources, scans, checker inputs, and facts
   * until a change. Like `fixedEntries`, a compilation that finds the project empty prepares the
   * files that these absolute paths reach. Ignored when `fixedEntries` is present.
   * @param shared - State to share with the other environments of the project, used only with
   * `fixedEntries` or `watchedEntries`. Its TypeScript settings must equal the settings that `root`
   * and `config` select. The caller owns and closes it.
   * @param domain - Name of this environment's resolver, which must differ between environments
   * that use `shared`.
   * @throws If the selected TypeScript configuration does not exist or native session creation fails.
   */
  constructor(
    root: string,
    loader: NativeDesignSystemLoader,
    config: BuildConfig,
    watch: boolean,
    mode: string,
    plugins?: PluginRuntime,
    fixedEntries?: readonly string[],
    watchedEntries?: readonly string[],
    shared?: SharedTypeFacts,
    domain?: string,
  ) {
    this.root = root;
    const fixed = Boolean(fixedEntries || watchedEntries);
    this.#shared = fixed ? shared : undefined;
    this.#domain = domain;
    this.#project = this.#shared?.project ?? (fixed ? new CompilationProject(true) : undefined);
    this.#entries = fixedEntries ?? watchedEntries ?? [];
    this.#typeFactsEnvironment = resolveTypeFactsEnvironment(root, config.tsconfig);
    this.native = new NativeProjectSession(loader, watch, mode, config.naming, false);
    attachPlugins(this.native, plugins);
  }

  /**
   * Compiles a module and records its resolver identities after compilation succeeds.
   *
   * @param code - Entry text before Stillwind transformation.
   * @param id - Full adapter resolver identity, including any query suffix.
   * @param host - Module descriptors, resolution, reads, and dependency tracking operations.
   * @param transport - Diagnostics, checker inputs, and generated imports for this compilation.
   * @returns The entry output, or undefined when compilation produces no entry code.
   * @throws If module identities conflict or host, checker, or compiler operations fail.
   */
  async compile(
    code: string,
    id: string,
    host: CompilerHost,
    transport: ModuleCompileOptions = {},
  ) {
    try {
      return await this.#compile(code, id, { ...host, resolverDomain: this.#domain }, transport);
    } catch (error) {
      if (!(error instanceof ResolutionMismatchError) || !this.#shared) {
        throw error;
      }
      // Facts collected in the shared project followed another environment's imports. This
      // environment continues with its own project and checker.
      this.#shared = undefined;
      this.#project = new CompilationProject(true);
      this.#preparation = undefined;
      this.#preparing = false;
      return this.#compile(code, id, host, transport);
    }
  }

  /**
   * Compiles a module with the current project and checker and records its resolver identities.
   *
   * @param code - Entry text before Stillwind transformation.
   * @param id - Full adapter resolver identity, including any query suffix.
   * @param host - Host whose resolver domain matches the current project.
   * @param transport - Diagnostics, checker inputs, and generated imports for this compilation.
   * @returns The entry output, or undefined when compilation produces no entry code.
   * @throws If module identities conflict, the shared project receives a different import answer,
   * or host, checker, or compiler operations fail.
   */
  async #compile(code: string, id: string, host: CompilerHost, transport: ModuleCompileOptions) {
    // Loading every file that the entries reach lets later compilations share one checker
    // project. Adding files after a checker query would replace the checker snapshot, so
    // compilations that start while the entries load wait for them before they query the checker.
    // The project is empty before the first compilation and after a closed checker discarded its
    // sources, so the entries load again then.
    const entries = this.#entries;
    const project = this.#project;
    const waiting = this.#preparation;
    if (project && entries.length > 0 && !this.#preparing && project.sourceKeys().length === 0) {
      this.#preparing = true;
      this.#preparation = (async () => {
        // The host resolver gives each path the identity spelling that its compilations use. The
        // entries only speed up later compilations, so an entry that does not resolve is skipped.
        const resolved = await Promise.all(
          entries.map((entry) => host.resolve(entry, id, { skipSelf: true }).catch(() => null)),
        );
        await project.prepare(
          resolved.flatMap((entry) => (entry && !entry.external ? [entry.id] : [])),
          host,
          this.native,
        );
      })().finally(() => {
        this.#preparing = false;
      });
      // The compilation that loads the entries reports their failure.
      await this.#preparation;
    } else {
      await waiting?.catch(() => {});
    }
    await project?.prepare([id], host, this.native);
    const pending = new Map<string, string>();
    const pendingByResolverId = new Map<string, string>();
    const output = await compileModule(
      this.native,
      () => this.#checker(),
      code,
      id,
      {
        ...host,
        onModule: (moduleId, resolverId) => {
          const previous = this.#resolverIdsByModuleId.get(moduleId);
          if (previous !== undefined && previous !== resolverId) {
            throw new Error(
              `Module identity ${moduleId} changed resolver identity from ${previous} to ${resolverId}`,
            );
          }
          const priorModuleId =
            pendingByResolverId.get(resolverId) ?? this.#moduleIdsByResolverId.get(resolverId);
          if (priorModuleId !== undefined && priorModuleId !== moduleId) {
            throw new Error(`Resolver identity ${resolverId} changed module identity`);
          }
          pending.set(moduleId, resolverId);
          pendingByResolverId.set(resolverId, moduleId);
        },
      },
      transport,
      this.#project,
    );
    for (const [moduleId, resolverId] of pending) {
      this.#resolverIdsByModuleId.set(moduleId, resolverId);
      this.#moduleIdsByResolverId.set(resolverId, moduleId);
      host.onModule?.(moduleId, resolverId);
    }
    return output;
  }

  /**
   * Starts the checker process that later compilations query, unless it is running. Compilations
   * start it on demand, so a caller uses this only to start it before the first compilation.
   *
   * @throws If the checker process cannot start.
   */
  startChecker(): void {
    this.#checker();
  }

  /**
   * Returns the host identity recorded after a compilation returned successfully.
   *
   * @param moduleId - Compiler identity previously observed during this session's compilation.
   * @returns Full resolver identity, including any query suffix.
   * @throws If this session has not recorded the module identity.
   */
  resolverId(moduleId: string): string {
    const id = this.#resolverIdsByModuleId.get(moduleId);
    if (id === undefined) {
      throw new Error(`Unknown Stillwind module identity: ${moduleId}`);
    }
    return id;
  }

  /** Returns the checker of the shared project or of this session, starting it on first use. */
  #checker(): TypeScriptFactsService {
    if (this.#shared) {
      return this.#shared.facts();
    }
    this.#facts ??= new TypeScriptFactsService(this.#typeFactsEnvironment);
    return this.#facts;
  }

  /**
   * Discards shared sources read from a file and every collected fact.
   *
   * @param file - Absolute path of a file that changed, was added, or was removed.
   * @param entry - Whether the file was added or removed rather than changed.
   */
  sourceChanged(file: string, entry: boolean): void {
    if (this.#shared) {
      this.#shared.sourceChanged(file, entry);
      return;
    }
    this.#project?.invalidateFile(file, entry);
  }

  /**
   * Discards the shared sources read from a file when they hold text other than the file's current
   * text, as {@link sourceChanged} does for a changed file. A caller that cannot learn about changes
   * before a compilation reads a file reports the text that it read.
   *
   * @param file - Absolute file path.
   * @param text - Text that the caller read from the file.
   */
  sourceRead(file: string, text: string): void {
    const captured = (this.#shared?.project ?? this.#project)?.source(file);
    if (captured && captured.document.code !== text) {
      this.sourceChanged(file, false);
    }
  }

  /**
   * Compares every shared source read from a file with the file's current text, and discards the
   * sources whose file changed or no longer exists, as {@link sourceChanged} does. A caller that
   * missed change reports uses it before the next compilation. Files added since the sources were
   * read are not found, and a file that only query-qualified sources read is not compared.
   *
   * @param read - Reads a file, or returns undefined when it does not exist.
   * @returns Completion of the comparison.
   */
  async verifySources(read: (file: string) => Promise<string | undefined>): Promise<void> {
    const project = this.#shared?.project ?? this.#project;
    if (!project) {
      return;
    }
    await Promise.all(
      project.sourceKeys().map(async (key) => {
        const source = project.source(key);
        // A query-qualified source holds a part of its file. Discarding the file's own source
        // also discards it.
        if (!source || cleanModuleId(source.resolverId) !== source.resolverId) {
          return;
        }
        const text = await read(source.resolverId).catch(() => undefined);
        if (text === undefined) {
          this.sourceChanged(source.resolverId, true);
        } else if (text !== source.document.code) {
          this.sourceChanged(source.resolverId, false);
        }
      }),
    );
  }

  /**
   * Tests whether a changed file exposes the same exports to other modules as before the change.
   * When it does, compilations of the modules that import it give the same output as before,
   * except for their checker diagnostics.
   *
   * @param file - Absolute path that {@link sourceChanged} received with `entry` false. A
   * compilation must have prepared the file again since then.
   * @returns False when the file was not compiled before and after the change, no other module
   * imports it, or its exports cannot be compared.
   * @throws If checker input installation fails.
   */
  keepsInterface(file: string): boolean {
    const facts = this.#shared ? this.#shared.facts() : this.#facts;
    return Boolean(facts && this.#project?.keepsInterface(file, facts));
  }

  /**
   * Closes the current checker while retaining native compiler state and selected TypeScript settings.
   * Later compilation can start a new checker. Repeated calls have no effect until that happens.
   *
   * @throws If checker cleanup fails.
   */
  close(): void {
    if (!this.#shared) {
      this.#project?.invalidate();
    }
    this.#preparation = undefined;
    this.#preparing = false;
    this.#facts?.close();
    this.#facts = undefined;
  }
}
