import path from "node:path";

import {
  NativeProjectSession,
  type BindingReport,
  type NativeDesignSystemLoader,
} from "@stillwind/native";
import type { StillwindOptions } from "stillwind/config";

import { analyzeModule, type CompilerHost, type ModuleAnalysis } from "../compilation/index.ts";
import { loadCssGraph, type CssHost } from "../css/index.ts";
import {
  TypeScriptFactsService,
  type TypeFactsBackend,
  type TypeFactsEnvironment,
  type TypeScriptFactsStats,
} from "../semantics/index.ts";
import { mapAnalysisReports } from "./diagnostic-map.ts";
import { compilerOptions, cssEntrypoint } from "./options.ts";

/**
 * Loads compiler inputs and CSS for one Node analysis project. AnalysisProject serializes calls
 * and installs captured document contents before asking this backend to resolve or load modules.
 */
export interface AnalysisHost extends CompilerHost {
  supports(id: string): boolean;
  /** Absolute project directory used for identities, CSS paths, and generated declarations. */
  root: string;
  options: StillwindOptions;
  css: CssHost;
  /** Fixed TypeScript settings selected when this host opens. */
  readonly typeFactsEnvironment: TypeFactsEnvironment;
  /** False disables additional semantic queries for this project. */
  readonly typeFacts?: false;
  /**
   * Replaces all buffers used by subsequent source and CSS reads.
   *
   * @param documents - Absolute file paths to captured text. Missing entries use backend reads.
   */
  setDocuments(documents: ReadonlyMap<string, string>): Promise<void>;
  /** Discards backend caches after disk changes. It must not overlap resolution or loads. */
  invalidate(): Promise<void>;
  /** Releases backend resources after its pending operations have completed. */
  close(): Promise<void>;
}

/** Result of an unpublished analysis at one project revision. */
export interface AnalysisResult {
  entry: string;
  revision: number;
  status: "complete" | "incomplete" | "stale" | "cancelled";
  reports: BindingReport[];
  graph?: ModuleAnalysis;
  error?: string;
}

/** Owns overlays, a reusable checker, and serialized transactions for one host. */
export class AnalysisProject {
  readonly host: AnalysisHost;

  readonly #backend?: () => TypeFactsBackend;
  readonly #documents = new Map<string, { source: string; version: number }>();

  #semantics?: TypeFactsBackend;
  #revision = 0;
  #queue: Promise<unknown> = Promise.resolve();
  #facts?: TypeScriptFactsService;
  #loader?: NativeDesignSystemLoader;
  #dirty = true;
  #cssDirty = true;
  #diskDirty = true;
  #closed = false;

  /**
   * Takes ownership of a backend without loading CSS or starting a checker.
   *
   * @param host - Initialized backend whose TypeScript environment uses the same root. Release
   * this project with `close` when its analysis services are no longer needed.
   * @param backend - Optional lazy semantic backend factory. Omission selects the Node checker.
   */
  constructor(host: AnalysisHost, backend?: () => TypeFactsBackend) {
    this.host = host;
    this.#backend = backend;
  }

  /** Returns a copy of checker counters when semantic analysis has started the checker. */
  get checkerStats(): TypeScriptFactsStats | undefined {
    return this.#facts && { ...this.#facts.stats };
  }

  /** Stores text at an increasing version and makes pending results stale. */
  update(id: string, source: string, version: number): void {
    id = path.resolve(id);
    const previous = this.#documents.get(id);
    if (previous?.version === version && previous.source === source) {
      return;
    }
    if (!Number.isSafeInteger(version) || (previous && previous.version >= version)) {
      throw new Error("Analysis document versions must increase");
    }
    this.#documents.set(id, {
      source,
      version,
    });
    this.#revision++;
    this.#dirty = true;
    this.#cssDirty ||= id.endsWith(".css");
  }

  /** Removes an unsaved buffer so subsequent transactions use the disk file. */
  closeDocument(id: string): void {
    id = path.resolve(id);
    if (this.#documents.delete(id)) {
      this.#revision++;
      this.#dirty = true;
      this.#cssDirty ||= id.endsWith(".css");
    }
  }

  /** Makes pending results stale; the next transaction rebuilds CSS and backend caches. */
  invalidate(): void {
    this.#revision++;
    this.#dirty = true;
    this.#cssDirty = true;
    this.#diskDirty = true;
  }

  /**
   * Queues analysis using the document contents and revision captured when this method is called.
   *
   * @param entry - Full resolver identity of a supported target.
   * @param signal - Optional cancellation checked before and after queued work. Cancellation
   * discards results without interrupting a checker query that has already started.
   * @returns Reports for the entry and demanded semantic dependencies. Host failures appear as
   * incomplete results with error text. Unavailable semantic information appears in the graph's
   * unverified ranges. Stale and cancelled results omit reports and graphs.
   */
  analyze(entry: string, signal?: AbortSignal): Promise<AnalysisResult> {
    const revision = this.#revision;
    const documents = new Map([...this.#documents].map(([id, document]) => [id, document.source]));
    const task = this.#queue.then(async (): Promise<AnalysisResult> => {
      const before = this.#discardReason(revision, signal);
      if (before) {
        return discardedResult(entry, revision, before);
      }
      const result = await this.#analyzeSnapshot(entry, revision, documents);
      const after = this.#discardReason(revision, signal);
      return after ? discardedResult(entry, revision, after) : result;
    });

    this.#queue = task.catch(() => {});
    return task;
  }

  /**
   * Marks the project closed, discards pending results, and waits for queued work before cleanup.
   * Repeated calls return without requesting further cleanup.
   *
   * @throws If checker or backend cleanup fails.
   */
  async close(): Promise<void> {
    if (this.#closed) {
      return;
    }

    this.#closed = true;
    this.#revision++;
    await this.#queue;

    const results = await Promise.allSettled([
      Promise.resolve().then(() => this.#semantics?.close()),
      Promise.resolve().then(() => this.host.close()),
    ]);
    const errors = results.flatMap((result) =>
      result.status === "rejected" ? [result.reason] : [],
    );
    if (errors.length) {
      throw new AggregateError(errors, "Failed to close Stillwind analysis services");
    }
  }

  #discardReason(revision: number, signal?: AbortSignal): "cancelled" | "stale" | undefined {
    if (signal?.aborted) {
      return "cancelled";
    }
    if (revision !== this.#revision) {
      return "stale";
    }
    return undefined;
  }

  async #analyzeSnapshot(
    entry: string,
    revision: number,
    documents: ReadonlyMap<string, string>,
  ): Promise<AnalysisResult> {
    const result: AnalysisResult = { entry, revision, status: "incomplete", reports: [] };
    try {
      if (this.#closed) {
        throw new Error("Analysis project is closed");
      }
      await this.host.setDocuments(documents);
      await this.#resetCaches();
      if (!this.host.supports?.(entry)) {
        throw new Error(`The ${this.host.constructor.name} host cannot analyze ${entry}`);
      }
      const loader = await this.#designSystem();
      const session = new NativeProjectSession(
        loader,
        false,
        "development",
        this.host.options.naming,
      );
      const graph = await analyzeModule(session, () => this.#semanticBackend(), entry, this.host, {
        overlayFiles: documents,
        ambientDeclarations: this.#ambientDeclarations(loader),
      });
      result.graph = graph;
      result.reports = await mapAnalysisReports(
        graph.batches.flatMap((batch) => batch.reports),
        graph.sources,
        (id) => this.host.read(id),
      );
      result.status = analysisComplete(graph) ? "complete" : "incomplete";
    } catch (error) {
      result.error = String(error);
    }
    return result;
  }

  async #resetCaches(): Promise<void> {
    if (!this.#dirty) {
      return;
    }
    this.#dirty = false;
    if (this.#diskDirty) {
      this.#diskDirty = false;
      await this.#semantics?.invalidate();
    }
    if (this.#cssDirty) {
      this.#cssDirty = false;
      this.#loader = undefined;
    }
    await this.host.invalidate();
  }

  async #designSystem(): Promise<NativeDesignSystemLoader> {
    if (!this.#loader) {
      const css = path.resolve(this.host.root, cssEntrypoint(this.host.options));
      this.#loader = (
        await loadCssGraph(css, compilerOptions(this.host.options), this.host.css)
      ).loader;
    }
    return this.#loader;
  }

  #ambientDeclarations(loader: NativeDesignSystemLoader): Map<string, string> {
    const declarations = new Map<string, string>();
    if (this.host.options.types !== false) {
      declarations.set(
        path.resolve(this.host.root, this.host.options.types ?? "src/stillwind.generated.d.ts"),
        loader.generateVocabularyTypes(),
      );
    }
    return declarations;
  }

  #semanticBackend(): TypeFactsBackend | undefined {
    if (this.host.typeFacts === false) {
      return undefined;
    }
    if (!this.#semantics) {
      const backend = this.#backend?.();
      if (backend) {
        this.#semantics = backend;
      } else {
        this.#facts = new TypeScriptFactsService(this.host.typeFactsEnvironment);
        this.#semantics = this.#facts;
      }
    }
    return this.#semantics;
  }
}

function discardedResult(
  entry: string,
  revision: number,
  status: "stale" | "cancelled",
): AnalysisResult {
  return {
    entry,
    revision,
    status,
    reports: [],
  };
}

function analysisComplete(graph: ModuleAnalysis): boolean {
  return (
    graph.unverified.length === 0 &&
    !graph.dependencies.some(
      (dependency) => dependency.status === "unresolved" || dependency.status === "unsupported",
    )
  );
}
