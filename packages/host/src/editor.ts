import { NativeProjectSession, type NativeDesignSystemLoader } from "@stillwind/native";

import { cleanModuleId, compileModule, type CompilerHost } from "./compiler.ts";
import type { CompilerOutput } from "./protocol.ts";
import { TypeScriptFactsService, type TypeScriptFactsStats } from "./type-facts.ts";

export interface EditorDocument {
  id: string;
  version: number;
  source: string;
}

export interface EditorAnalysis {
  document: EditorDocument;
  revision: number;
  coverage: "observed";
  status: "success" | "partial" | "stale" | "cancelled";
  output?: CompilerOutput;
  error?: string;
  dependencies: string[];
  checkerStats?: TypeScriptFactsStats;
  previousSuccess?: {
    version: number;
    revision: number;
    output: CompilerOutput;
  };
}

/** Versioned, unpublished previews. Each request owns its compiler session. */
export class EditorProject {
  readonly #documents = new Map<string, EditorDocument>();
  readonly #successes = new Map<string, NonNullable<EditorAnalysis["previousSuccess"]>>();

  #revision = 0;

  readonly root: string;
  readonly host: CompilerHost;
  readonly designSystem: () => NativeDesignSystemLoader;

  constructor(root: string, host: CompilerHost, designSystem: () => NativeDesignSystemLoader) {
    this.root = root;
    this.host = host;
    this.designSystem = designSystem;
  }

  update(document: EditorDocument): void {
    const id = cleanModuleId(document.id);
    const previous = this.#documents.get(id);
    if (
      !Number.isSafeInteger(document.version) ||
      (previous && document.version <= previous.version)
    ) {
      throw new Error("Editor document versions must increase monotonically");
    }
    this.#documents.set(id, { ...document, id });
    this.#revision += 1;
  }

  close(id: string): void {
    id = cleanModuleId(id);
    if (this.#documents.delete(id)) {
      this.#revision += 1;
    }
    this.#successes.delete(id);
  }

  /** Call after disk dependencies or the design system change. */
  invalidate(): void {
    this.#revision += 1;
  }

  async analyze(id: string, signal?: AbortSignal): Promise<EditorAnalysis> {
    id = cleanModuleId(id);
    const document = this.#documents.get(id);
    if (!document) {
      throw new Error(`Editor document is not open: ${id}`);
    }
    const revision = this.#revision;
    const overlays = new Map(this.#documents);
    const dependencies = new Set<string>();
    const result: EditorAnalysis = {
      document: { ...document },
      revision,
      coverage: "observed",
      status: "partial",
      dependencies: [],
      previousSuccess: this.#successes.get(id),
    };
    const assertActive = () => {
      if (signal?.aborted) {
        throw new Error("Editor analysis cancelled");
      }
      if (revision !== this.#revision) {
        throw new Error("Editor analysis superseded");
      }
    };
    let facts: TypeScriptFactsService | undefined;
    try {
      assertActive();
      const session = new NativeProjectSession(this.designSystem(), false);
      const output = await compileModule(
        session,
        () => {
          facts ??= new TypeScriptFactsService(this.root);
          return facts;
        },
        document.source,
        id,
        this.root,
        {
          resolve: async (...args) => {
            assertActive();
            const resolved = await this.host.resolve(...args);
            assertActive();
            return resolved;
          },
          read: async (dependency) => {
            assertActive();
            const source =
              overlays.get(cleanModuleId(dependency))?.source ?? (await this.host.read(dependency));
            assertActive();
            return source;
          },
          watch: (dependency) => dependencies.add(dependency),
          warn: () => {},
        },
        { diagnostics: "collect" },
      );
      assertActive();
      result.output = output;
      const denied = output?.lint_findings.some(
        (finding) => output.lint_policy[finding.rule] === "deny",
      );
      if (output?.code != null && output.diagnostics.length === 0 && !denied) {
        result.status = "success";
        this.#successes.set(id, {
          version: document.version,
          revision,
          output,
        });
      }
    } catch (error) {
      result.error = error instanceof Error ? error.message : String(error);
    } finally {
      if (facts) {
        result.checkerStats = { ...facts.stats };
      }
      facts?.close();
      result.dependencies = [...dependencies];
    }
    if (signal?.aborted || revision !== this.#revision) {
      result.status = signal?.aborted ? "cancelled" : "stale";
      delete result.output;
    }
    return result;
  }
}
