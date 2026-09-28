import { compileModule, TypeScriptFactsService, type CompilerHost } from "@stillwind/host";
import { NativeProjectSession, type NativeDesignSystemLoader } from "@stillwind/native";

import type { StillwindOptions } from "./options.ts";

/** Owns compiler and lazy type facts for one adapter environment or reconstruction. */
export class IntegrationSession {
  readonly native: NativeProjectSession;
  readonly root: string;
  readonly #resolverIdsByModuleId = new Map<string, string>();
  readonly #moduleIdsByResolverId = new Map<string, string>();
  private facts?: TypeScriptFactsService;

  constructor(
    root: string,
    loader: NativeDesignSystemLoader,
    options: StillwindOptions,
    watch: boolean,
    mode: string,
  ) {
    this.root = root;
    this.native = new NativeProjectSession(loader, watch, mode, options.naming);
  }

  async compile(
    code: string,
    id: string,
    host: CompilerHost,
    transport: Parameters<typeof compileModule>[6] = {},
  ) {
    const pending = new Map<string, string>();
    const pendingByResolverId = new Map<string, string>();
    const output = await compileModule(
      this.native,
      () => (this.facts ??= new TypeScriptFactsService(this.root)),
      code,
      id,
      this.root,
      {
        ...host,
        onModule: (moduleId, resolverId) => {
          const previous = this.#resolverIdsByModuleId.get(moduleId);
          if (previous !== undefined && previous !== resolverId) {
            throw new Error(`Module identity ${moduleId} changed resolver identity`);
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

  /** Returns the host resolver identity for a published module. */
  resolverId(moduleId: string): string {
    const id = this.#resolverIdsByModuleId.get(moduleId);
    if (id === undefined) {
      throw new Error(`Unknown Stillwind module identity: ${moduleId}`);
    }
    return id;
  }

  close(): void {
    this.facts?.close();
    this.facts = undefined;
  }
}
