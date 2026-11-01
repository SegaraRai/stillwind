import path from "node:path";

import type { BindingReport } from "@stillwind/native";

import { analyzeModule, type ModuleAnalysis } from "../../compilation/index.ts";
import { CompilationProject, sourceKey } from "../../compilation/project.ts";
import { isStylesheet } from "../../css/index.ts";
import { queryPluginPaths } from "../../css/plugins/paths.ts";
import { prepareSourcePlugins } from "../../css/plugins/session.ts";
import { EditorSnapshot, type EditorRequest, type EditorResult } from "../../editor/index.ts";
import {
  TypeScriptFactsService,
  type TypeFactsBackend,
  type TypeScriptFactsStats,
} from "../../semantics/index.ts";
import { mapAnalysisReports } from "../diagnostic-map.ts";
import { StylesheetEditor } from "../stylesheet-editor.ts";
import { AnalysisCss, type AnalysisHost } from "./host.ts";

/**
 *  Result of an unpublished analysis at one project revision.
 */
export interface AnalysisResult {
  /** Entry resolver identity supplied to analyze. */
  entry: string;
  /** Project change counter captured when analysis was queued. */
  revision: number;
  /**
   * Complete describes available analysis inputs, even if reports contain compiler errors.
   * Incomplete retains partial findings or an exception. Stale and cancelled omit reports and graph.
   */
  status: "complete" | "incomplete" | "stale" | "cancelled";
  /** Findings mapped to original document URIs and zero-based UTF-16 positions. */
  reports: BindingReport[];
  /** Observed semantic graph, including dependency status and unavailable facts. Omitted on discard or early failure. */
  graph?: ModuleAnalysis;
  /** Caught exception as text. Compiler reports alone do not populate this field. */
  error?: string;
}

/**
 * Owns open buffers, checker queries, and serialized analysis for one host.
 */
export class AnalysisProject {
  /** Prepared sources shared by analyses of different entries. */
  readonly #compilation = new CompilationProject();

  /** CSS state and dependencies owned by this project. */
  readonly #css: AnalysisCss;

  /** Owned host. The project serializes its operations and closes it during cleanup. */
  readonly host: AnalysisHost;

  /** Optional factory used once when semantic analysis first needs a backend. */
  readonly #backend?: () => TypeFactsBackend;

  /** Unsaved original buffers keyed by absolute path and captured before each queued operation. */
  readonly #documents = new Map<
    string,
    {
      /** Complete original document text. */
      source: string;
      /** Last accepted safe integer version while the path is open. */
      version: number;
    }
  >();

  /** Editor snapshots retained by entry identity until their project revision changes. */
  readonly #editors = new Map<
    string,
    {
      /** Project change counter represented by this snapshot. */
      revision: number;
      /** Immutable view used for editor queries: a script graph or one linted stylesheet. */
      snapshot: EditorSnapshot | StylesheetEditor;
      /** True when CSS definitions were recovered and structural edits require strict analysis. */
      recovered?: boolean;
    }
  >();

  /** Lazily selected custom or Node semantic backend owned by this project. */
  #semantics?: TypeFactsBackend;

  /** Change counter used to discard pending analyses and editor answers. */
  #revision = 0;

  /** Serializes transactions and absorbs failures before subsequent work. */
  #queue: Promise<unknown> = Promise.resolve();

  /** Node backend retained separately to expose checker statistics. */
  #facts?: TypeScriptFactsService;

  /** Requests backend invalidation before the next transaction. */
  #dirty = true;

  /** Requests replacement of the cached CSS loader. */
  #cssDirty = true;

  /** Requests semantic disk-cache invalidation after an external change. */
  #diskDirty = true;

  /** Prevents transactions from producing new results during cleanup. */
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
    this.#css = new AnalysisCss(host);
    this.#backend = backend;
  }

  /**
   * Reads counters from the owned Node checker without exposing its mutable statistics object.
   * @returns A copy of the counters, or undefined before checker creation or with a custom backend.
   */
  get checkerStats(): TypeScriptFactsStats | undefined {
    return this.#facts && { ...this.#facts.stats };
  }

  /**
   * Stores an unsaved buffer. Changed text advances the revision and makes pending results stale.
   *
   * @param id - File path resolved from the process working directory when relative.
   * @param source - Complete original document text.
   * @param version - Safe integer greater than the open buffer's version. An identical buffer and
   * version have no effect. Identical text with a newer version updates only the version.
   * A document can restart its version sequence after closeDocument.
   * @throws If the version is invalid or does not increase for a changed open buffer.
   */
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
    if (previous?.source === source) {
      return;
    }
    if (this.#compilation.source(id)?.document.code !== source) {
      this.#compilation.invalidate(id);
    }
    this.#revision++;
    this.#dirty = true;
    this.#cssDirty ||= this.#css.affects(id);
  }

  /**
   * Removes an unsaved buffer and advances the revision when one existed.
   *
   * @param id - File path, resolved in the same way as update. Later reads use the host's source
   * provider. Unknown paths have no effect.
   */
  closeDocument(id: string): void {
    id = path.resolve(id);
    if (this.#documents.delete(id)) {
      this.#compilation.invalidate(id);
      this.#revision++;
      this.#dirty = true;
      this.#cssDirty ||= this.#css.affects(id);
    }
  }

  /**
   * Lists the files that analysis has observed, for comparison with paths in watch notifications.
   *
   * @returns Keys from `sourceKey` for prepared sources, files that CSS or plugins read, and paths
   * that the checker read or tested, including missing paths. A custom semantic backend contributes
   * nothing.
   */
  reads(): Set<string> {
    return new Set([
      ...this.#compilation.sourceKeys(),
      ...[...this.#css.dependencies, ...(this.#facts?.dependencies ?? [])].map(sourceKey),
    ]);
  }

  /** Makes pending results stale. The next transaction rebuilds CSS and backend caches. */
  invalidate(): void {
    this.#compilation.invalidate();
    this.#revision++;
    this.#dirty = true;
    this.#cssDirty = true;
    this.#diskDirty = true;
  }

  /**
   * Lists the stylesheets that this project lints, using the document contents captured when this
   * method is called.
   *
   * @param editor - When false (the default), loads the original CSS and rejects invalid input.
   * When true, retries a failed load with recovered definitions so incomplete CSS can still
   * determine membership for completion, hover, and colors.
   * @returns Identities of the CSS entry and the imported files that the CSS host marks for lint,
   * in load order. Package CSS is excluded unless the host marks it. Returns an empty list
   * if the captured buffers become outdated before loading starts.
   * @throws If the project is closed, installing buffers or invalidating caches fails, or the
   * selected CSS loading mode fails.
   */
  stylesheets(editor = false): Promise<string[]> {
    const revision = this.#revision;
    const documents = new Map([...this.#documents].map(([id, document]) => [id, document.source]));
    const task = this.#queue.then(async () => {
      if (this.#closed) {
        throw new Error("Analysis project is closed");
      }
      if (!(await this.#installDocuments(documents, revision))) {
        return [];
      }

      if (editor) {
        return [...(await this.#css.editor()).lintSources];
      }
      await this.#css.load();
      return [...this.#css.lintSources];
    });
    this.#queue = task.catch(() => {});
    return task;
  }

  /**
   * Queues analysis using the document contents and revision captured when this method is called.
   *
   * @param entry - Full resolver identity of a supported target, or of a stylesheet that
   * {@link stylesheets} lists. A stylesheet's reports are its stylesheet lint findings, and its
   * result has no graph. Membership filtering is performed by AnalysisWorkspace, not by this
   * project.
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
   * Loads targets together and prepares plugin candidates using the current buffers.
   *
   * @param entries - Full resolver IDs. The workspace must apply project membership rules first.
   * @returns Completion of preparation. Closed or outdated requests stop without publishing results.
   * Loading and plugin failures are left for each target's analysis to report. A failed plugin
   * runtime is discarded before retrying.
   */
  prepare(entries: readonly string[]): Promise<void> {
    const revision = this.#revision;
    const documents = new Map([...this.#documents].map(([id, document]) => [id, document.source]));
    const task = this.#queue.then(async () => {
      if (this.#closed || this.#discardReason(revision)) {
        return;
      }
      try {
        if (!(await this.#installDocuments(documents, revision))) {
          return;
        }

        const loader = await this.#css.load();
        if (this.#discardReason(revision)) {
          return;
        }
        await this.#compilation.prepare(entries, this.host, this.#css.session!);
        await prepareSourcePlugins(this.#css.session!, loader, this.#compilation.texts());
      } catch {
        this.#css.discardPlugins();
        // Scoped analysis reports loading and preparation failures for their original targets.
      }
    });
    this.#queue = task.catch(() => {});
    return task;
  }

  /**
   * Queries the captured analysis for an entry. Buffer changes or cancellation discard its answer.
   * CSS completion, hover, resolution, and colors can load recovered definitions for incomplete
   * input. CSS sorting and lint fixes require the original graph to load successfully.
   * Plugin path completion and links read captured CSS inside the project root without loading
   * its graph or executing plugins.
   *
   * @param entry - Full resolver identity. The workspace applies target membership filtering.
   * @param request - Operation with offsets in the original UTF-16 document.
   * @param signal - Optional cancellation checked before and after queued work.
   * @returns The current editor answer, or undefined after closure, cancellation, a revision change,
   * failed entry analysis, or a semantic CSS query outside the graph's stylesheets marked for lint.
   * @throws If preparing a CSS query fails or the editor query rejects a candidate or preview.
   */
  query(
    entry: string,
    request: EditorRequest,
    signal?: AbortSignal,
  ): Promise<EditorResult | undefined> {
    const revision = this.#revision;
    const documents = new Map([...this.#documents].map(([id, document]) => [id, document.source]));
    const task = this.#queue.then(async () => {
      if (this.#closed || this.#discardReason(revision, signal)) {
        return undefined;
      }
      if (isStylesheet(entry) && ["completion", "resolve", "links"].includes(request.kind)) {
        if (!(await this.#installDocuments(documents, revision, signal))) {
          return undefined;
        }
        const text = await this.host.css.read(entry).catch(() => undefined);
        if (text === undefined) {
          return undefined;
        }
        const navigation = await queryPluginPaths(entry, text, request, {
          root: this.host.root,
          names: this.host.config.plugins?.map((plugin) => plugin.name) ?? [],
          host: this.host.css.plugins,
          documentUri: (id) => this.host.css.documentUri(id),
        });
        if (navigation) {
          return this.#discardReason(revision, signal) ? undefined : navigation;
        }
      }
      const structural = request.kind === "sort" || request.kind === "fixes";
      if (structural && this.#editors.get(entry)?.recovered) {
        this.#editors.delete(entry);
      }
      if (this.#editors.get(entry)?.revision !== revision) {
        if (
          isStylesheet(entry) &&
          ["completion", "hover", "resolve", "colors", "colorPresentation"].includes(request.kind)
        ) {
          if (!(await this.#installDocuments(documents, revision, signal))) {
            return undefined;
          }
          const editor = await this.#css.editorSnapshot(entry);
          if (!editor) {
            return undefined;
          }
          this.#editors.set(entry, {
            revision,
            ...editor,
          });
        } else {
          await this.#analyzeSnapshot(entry, revision, documents);
        }
      }
      const snapshot = this.#editors.get(entry)?.snapshot;
      await snapshot?.prepare(entry, request);
      const result = snapshot?.query(entry, request);
      return this.#discardReason(revision, signal) ? undefined : result;
    });
    this.#queue = task.catch(() => {});
    return task;
  }

  /**
   * Marks the project closed, discards pending results, and waits for queued work before cleanup.
   * Repeated calls return without requesting further cleanup.
   *
   * @throws If checker or backend cleanup fails.
   * @returns A promise that resolves after queued analysis and cleanup. Repeated calls made after
   * closure starts return immediately. They do not wait for the first call's cleanup.
   */
  async close(): Promise<void> {
    if (this.#closed) {
      return;
    }

    this.#closed = true;
    this.#editors.clear();
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

  /**
   * Determines whether a captured operation can still publish its result.
   *
   * @param revision - Change counter captured when the operation was queued.
   * @param signal - Optional request cancellation, which takes precedence over staleness.
   * @returns The discard status, or undefined while the capture remains current.
   */
  #discardReason(revision: number, signal?: AbortSignal): "cancelled" | "stale" | undefined {
    if (signal?.aborted) {
      return "cancelled";
    }
    if (revision !== this.#revision) {
      return "stale";
    }
    return undefined;
  }

  /**
   * Installs captured buffers, refreshes caches, and retains an editor snapshot after compilation.
   *
   * @param entry - Supported full resolver identity to analyze.
   * @param revision - Change counter associated with the captured documents.
   * @param documents - Independent map of open absolute paths to original text.
   * @returns Findings mapped to original documents. Host failures become incomplete results with error text.
   */
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
      if (!(await this.#installDocuments(documents, revision))) {
        return result;
      }
      if (isStylesheet(entry)) {
        const stylesheet = await this.#css.stylesheet(entry);
        result.reports = stylesheet.reports;
        result.status = "complete";
        this.#editors.set(entry, {
          revision,
          snapshot: stylesheet.snapshot,
        });
        return result;
      }
      if (!this.host.supports?.(entry)) {
        throw new Error(`The ${this.host.constructor.name} host cannot analyze ${entry}`);
      }
      await this.#css.load();
      const loader = this.#css.loader!;
      const session = this.#css.session!;
      const graph = await analyzeModule(
        session,
        () => this.#semanticBackend(),
        entry,
        this.host,
        {
          overlayFiles: documents,
        },
        revision === this.#revision ? this.#compilation : undefined,
      );
      result.graph = structuredClone(graph);
      result.reports = await mapAnalysisReports(
        graph.batches.flatMap((batch) => batch.reports),
        graph.sources,
        (id) => this.host.read(id),
      );
      const editor = new EditorSnapshot(
        graph,
        session,
        loader,
        this.host.config,
        (await this.#css.plugins?.catalog()) ?? [],
        this.#css.plugins,
      );
      result.reports.push(...editor.reports());
      this.#editors.set(entry, {
        revision,
        snapshot: editor,
      });
      result.status = analysisComplete(graph) ? "complete" : "incomplete";
    } catch (error) {
      result.error = error instanceof Error ? error.message : String(error);
      if (error instanceof Error && "reports" in error && Array.isArray(error.reports)) {
        result.reports = error.reports as BindingReport[];
      }
    }
    return result;
  }

  /**
   * Installs captured buffers and refreshes caches before work continues.
   *
   * @param documents - Original buffers captured with the supplied revision.
   * @param revision - Revision that owns this queued operation.
   * @param signal - Optional cancellation checked between host operations.
   * @returns Whether the operation can continue with these buffers after cache refresh.
   * @throws If installing buffers or refreshing host or checker caches fails.
   */
  async #installDocuments(
    documents: ReadonlyMap<string, string>,
    revision: number,
    signal?: AbortSignal,
  ): Promise<boolean> {
    await this.host.setDocuments(documents);
    if (this.#discardReason(revision, signal)) {
      return false;
    }
    await this.#resetCaches();
    return !this.#discardReason(revision, signal);
  }

  /**
   * Consumes dirty flags and invalidates semantic, CSS, and host caches as requested.
   *
   * @returns Completion of required invalidation, or no work when the project is clean.
   * @throws If semantic or host invalidation fails.
   */
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
      this.#css.invalidate();
    }
    await this.host.invalidate();
  }

  /**
   * Lazily selects the custom backend or creates the Node checker with fixed project settings.
   *
   * @returns The owned backend, or undefined when type facts are disabled.
   * @throws If backend creation fails.
   */
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

/**
 * Creates a result that carries no reports or graph after a capture becomes unusable.
 *
 * @param entry - Requested resolver identity.
 * @param revision - Change counter captured when the request was queued.
 * @param status - Reason the capture was discarded.
 * @returns An empty analysis result preserving request identity and revision.
 */
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

/**
 * Checks whether all demanded semantic inputs were available, independently of compiler error reports.
 *
 * @param graph - Observed graph returned by compilation.
 * @returns Whether no ranges are unverified and no dependency is unresolved or unsupported.
 */
function analysisComplete(graph: ModuleAnalysis): boolean {
  return (
    graph.unverified.length === 0 &&
    !graph.dependencies.some(
      (dependency) => dependency.status === "unresolved" || dependency.status === "unsupported",
    )
  );
}
