import type { BuildConfig } from "@stillwind/config";
import {
  compileModule,
  resolveTypeFactsEnvironment,
  TypeScriptFactsService,
  type CompilerHost,
  type ModuleCompileOptions,
  type TypeFactsEnvironment,
} from "@stillwind/host";
import { NativeProjectSession, type NativeDesignSystemLoader } from "@stillwind/native";

/**
 * Owns a compiler session and a reusable checker for one adapter environment. TypeScript settings
 * are selected at construction. Close the checker when the adapter releases or reloads the project.
 */
export class IntegrationSession {
  /** Native compiler state retained across module updates. Closing this wrapper only releases its checker. */
  readonly native: NativeProjectSession;
  /** Absolute project directory captured when TypeScript settings were selected. */
  readonly root: string;

  readonly #typeFactsEnvironment: TypeFactsEnvironment;
  readonly #resolverIdsByModuleId = new Map<string, string>();
  readonly #moduleIdsByResolverId = new Map<string, string>();
  #facts?: TypeScriptFactsService;

  /**
   * Creates compiler state without starting the TypeScript checker.
   *
   * @param root - Absolute adapter project root used for module identities and TypeScript settings.
   * @param loader - Completed design-system loader retained by the native session.
   * @param config - Compiler settings, including the root-relative or absolute tsconfig selection.
   * @param watch - Whether failed native updates retain the last successful output.
   * @param mode - Native compilation mode selected by the adapter.
   * @throws If the selected TypeScript configuration does not exist or native session creation fails.
   */
  constructor(
    root: string,
    loader: NativeDesignSystemLoader,
    config: BuildConfig,
    watch: boolean,
    mode: string,
  ) {
    this.root = root;
    this.#typeFactsEnvironment = resolveTypeFactsEnvironment(root, config.tsconfig);
    this.native = new NativeProjectSession(loader, watch, mode, config.naming);
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
    this.#facts?.close();
    this.#facts = undefined;
  }
}
