import type { BuildConfig } from "@stillwind/config";
import {
  attachPlugins,
  CompilationProject,
  compileModule,
  compilerOptions,
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
   * Sources, scans, and checker inputs shared by compilations while source files stay unchanged.
   * Absent when files can change during the session.
   */
  readonly #project?: CompilationProject;
  /** Entries that the next compilation prepares together with its own entry. */
  #entries: readonly string[];

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
   * The first compilation prepares the files that these absolute paths reach, together with its own
   * entry, so later compilations seldom add files to the checker project.
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
  ) {
    this.root = root;
    this.#project = fixedEntries ? new CompilationProject(true) : undefined;
    this.#entries = fixedEntries ?? [];
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
    // Loading every file that the entries reach lets later compilations share one checker
    // project. Adding files after a checker query would replace the checker snapshot.
    const entries = this.#entries;
    this.#entries = [];
    // The host resolver gives each path the identity spelling that its compilations use. The
    // entries only speed up later compilations, so an entry that does not resolve is skipped.
    const resolved = await Promise.all(
      entries.map((entry) => host.resolve(entry, id, { skipSelf: true }).catch(() => null)),
    );
    await this.#project?.prepare(
      [...resolved.flatMap((entry) => (entry && !entry.external ? [entry.id] : [])), id],
      host,
      this.native,
    );
    const pending = new Map<string, string>();
    const pendingByResolverId = new Map<string, string>();
    const output = await compileModule(
      this.native,
      () => {
        this.#facts ??= new TypeScriptFactsService(this.#typeFactsEnvironment);
        return this.#facts;
      },
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

  /**
   * Closes the current checker while retaining native compiler state and selected TypeScript settings.
   * Later compilation can start a new checker. Repeated calls have no effect until that happens.
   *
   * @throws If checker cleanup fails.
   */
  close(): void {
    this.#project?.invalidate();
    this.#entries = [];
    this.#facts?.close();
    this.#facts = undefined;
  }
}
