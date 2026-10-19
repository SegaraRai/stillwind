import path from "node:path";
import { fileURLToPath } from "node:url";

import type { BaseConfig, PreviewOptions } from "@stillwind/config";
import {
  NativeProjectSession,
  type BindingReport,
  type NativeDesignSystemLoader,
} from "@stillwind/native";

import { analyzeModule, type CompilerHost, type ModuleAnalysis } from "../compilation/index.ts";
import {
  attachPlugins,
  isStylesheet,
  loadCssGraph,
  loadEditorCssGraph,
  type CssHost,
  type CssPluginInputs,
  type EditorCssGraph,
  type PluginRuntime,
} from "../css/index.ts";
import { queryPluginPaths } from "../css/plugin-paths.ts";
import { EditorSnapshot, type EditorRequest, type EditorResult } from "../editor/index.ts";
import {
  TypeScriptFactsService,
  type TypeFactsBackend,
  type TypeFactsEnvironment,
  type TypeScriptFactsStats,
} from "../semantics/index.ts";
import { mapAnalysisReports } from "./diagnostic-map.ts";
import { compilerOptions, cssEntrypoint } from "./options.ts";
import { StylesheetEditor } from "./stylesheet-editor.ts";

/**
 * Loads compiler inputs and CSS for one Node analysis project. AnalysisProject serializes calls
 * and installs captured document contents before asking this backend to resolve or load modules.
 */
export interface AnalysisHost extends CompilerHost {
  /**
   * Checks whether a resolved target can be analyzed by this host.
   *
   * @param id - Full resolver identity of the target.
   * @returns Whether load can supply supported source for the identity.
   */
  supports(id: string): boolean;
  /** Absolute project directory used for identities and CSS paths. */
  root: string;
  /**
   * Compiler and preview settings fixed for this host's lifetime. Relative CSS and tsconfig paths
   * use root.
   */
  config: BaseConfig & {
    preview?: PreviewOptions;
  };
  /** CSS resolution and reads using the same captured document buffers as source loading. */
  css: CssHost;
  /** Fixed TypeScript settings selected when this host opens. */
  readonly typeFactsEnvironment: TypeFactsEnvironment;
  /** False disables additional semantic queries for this project. */
  readonly typeFacts?: false;
  /**
   * Replaces all buffers used by subsequent source and CSS reads.
   *
   * @param documents - Absolute file paths to captured text. Missing entries use backend reads.
   * @returns A promise that resolves when subsequent reads can use the supplied snapshot.
   * @throws If the backend cannot install the supplied buffers.
   */
  setDocuments(documents: ReadonlyMap<string, string>): Promise<void>;
  /**
   * Discards backend caches before the next transaction. Must not overlap resolution or loads.
   *
   * @returns A promise that resolves when cached reads and resolutions have been invalidated.
   * @throws If backend invalidation fails.
   */
  invalidate(): Promise<void>;
  /**
   * Releases backend resources. The caller first waits for pending host operations.
   *
   * @returns A promise for cleanup completion.
   * @throws If backend cleanup fails.
   */
  close(): Promise<void>;
}

/** Result of an unpublished analysis at one project revision. */
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

/** Owns overlays, a reusable checker, and serialized transactions for one host. */
export class AnalysisProject {
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
  /** CSS loader retained until a CSS update or explicit invalidation. */
  #loader?: NativeDesignSystemLoader;
  /** Query-only CSS graph, rebuilt after CSS changes and kept separate from strict analysis. */
  #editorCss?: EditorCssGraph;
  /** Identities that the retained loader marked for stylesheet lint, in load order. */
  #lintSources: string[] = [];
  /**
   * Plugin runtime of the retained loader. A CSS update keeps it as the previous runtime of the
   * next load, so definitions of unchanged plugins survive.
   */
  #plugins?: PluginRuntime;
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
   * Stores an unsaved buffer and advances the revision used to discard pending results.
   *
   * @param id - File path resolved from the process working directory when relative.
   * @param source - Complete original document text.
   * @param version - Safe integer greater than the open buffer's version. An identical buffer and
   * version have no effect. A document can restart its version sequence after closeDocument.
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
    this.#revision++;
    this.#dirty = true;
    this.#cssDirty ||= isStylesheet(id);
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
      this.#revision++;
      this.#dirty = true;
      this.#cssDirty ||= isStylesheet(id);
    }
  }

  /** Makes pending results stale. The next transaction rebuilds CSS and backend caches. */
  invalidate(): void {
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
   * in load order. Package CSS is excluded unless the host marks it.
   * @throws If the project is closed, installing buffers or invalidating caches fails, or the
   * selected CSS loading mode fails.
   */
  stylesheets(editor = false): Promise<string[]> {
    const documents = new Map([...this.#documents].map(([id, document]) => [id, document.source]));
    const task = this.#queue.then(async () => {
      if (this.#closed) {
        throw new Error("Analysis project is closed");
      }
      await this.host.setDocuments(documents);
      await this.#resetCaches();
      if (editor) {
        return [...(await this.#editorDesignSystem()).lintSources];
      }
      await this.#designSystem();
      return [...this.#lintSources];
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
   * Queries the captured analysis for an entry. Buffer changes or cancellation discard its answer.
   * CSS completion, hover, resolution, and colors can load recovered definitions for incomplete
   * input. CSS sorting and lint fixes require the original graph to load successfully.
   * Plugin path completion and links read captured CSS inside the project root without loading
   * its graph or executing plugins.
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
        await this.host.setDocuments(documents);
        await this.#resetCaches();
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
          await this.host.setDocuments(documents);
          await this.#resetCaches();
          const graph = await this.#editorDesignSystem();
          if (!graph.lintSources.some((source) => sameFile(source, entry))) {
            return undefined;
          }
          const reports = graph.recovered
            ? []
            : graph.loader
                .stylesheetReports()
                .filter(
                  (report) => report.primary.location.uri === this.host.css.documentUri(entry),
                );
          this.#editors.set(entry, {
            revision,
            recovered: graph.recovered,
            snapshot: new StylesheetEditor(
              entry,
              await this.host.css.read(entry),
              reports,
              graph.loader.editorQuery(),
              this.host.config,
              (await graph.plugins?.catalog()) ?? [],
              graph.plugins,
            ),
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
      await this.host.setDocuments(documents);
      await this.#resetCaches();
      if (isStylesheet(entry)) {
        return await this.#analyzeStylesheet(entry, revision, result);
      }
      if (!this.host.supports?.(entry)) {
        throw new Error(`The ${this.host.constructor.name} host cannot analyze ${entry}`);
      }
      const loader = await this.#designSystem();
      const session = new NativeProjectSession(
        loader,
        false,
        "development",
        this.host.config.naming,
      );
      attachPlugins(session, this.#plugins);
      const graph = await analyzeModule(session, () => this.#semanticBackend(), entry, this.host, {
        overlayFiles: documents,
      });
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
        (await this.#plugins?.catalog()) ?? [],
        this.#plugins,
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
   * Reports stylesheet lint for one stylesheet of the current CSS graph and retains its fixes.
   * @param entry - Resolver identity of the stylesheet.
   * @param revision - Change counter associated with the installed documents.
   * @param result - Result to complete. Its reports are replaced.
   * @returns The completed result with reports whose primary location is the stylesheet.
   * @throws If resolving, reading, or loading the CSS graph fails.
   */
  async #analyzeStylesheet(
    entry: string,
    revision: number,
    result: AnalysisResult,
  ): Promise<AnalysisResult> {
    const loader = await this.#designSystem();
    result.reports = loader.stylesheetReports().filter((report) => {
      const uri = report.primary.location.uri;
      return uri.startsWith("file:")
        ? sameFile(fileURLToPath(uri), entry)
        : uri === this.host.css.documentUri(entry);
    });
    this.#editors.set(entry, {
      revision,
      snapshot: new StylesheetEditor(
        entry,
        await this.host.css.read(entry),
        result.reports,
        loader.editorQuery(),
        this.host.config,
        (await this.#plugins?.catalog()) ?? [],
        this.#plugins,
      ),
    });
    result.status = "complete";
    return result;
  }

  /**
   * Consumes dirty flags and invalidates semantic, CSS, and host caches as requested.
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
      this.#loader = undefined;
      this.#editorCss = undefined;
    }
    await this.host.invalidate();
  }

  /**
   * Loads the configured CSS graph once until CSS invalidation clears it.
   * @returns The retained loader with compiler options applied.
   * @throws If resolving, reading, or loading the CSS graph fails.
   */
  async #designSystem(): Promise<NativeDesignSystemLoader> {
    if (!this.#loader) {
      const css = path.resolve(this.host.root, cssEntrypoint(this.host.config));
      const loaded = await loadCssGraph(
        css,
        compilerOptions(this.host.config),
        this.host.css,
        undefined,
        undefined,
        this.#pluginInputs(),
      );
      this.#loader = loaded.loader;
      this.#lintSources = loaded.lintSources;
      this.#plugins = loaded.plugins;
    }
    return this.#loader;
  }

  /**
   * Selects the plugins of the project's CSS graph.
   * @returns Configured plugins, the project root, and the runtime of the last load.
   */
  #pluginInputs(): CssPluginInputs {
    return {
      plugins: this.host.config.plugins,
      root: this.host.root,
      previous: this.#plugins,
    };
  }

  /**
   * Loads a query graph from original CSS or recovered definitions, without publishing its reports.
   * @returns The cached editor graph, reusing the strict loader when available. Otherwise, a failed
   * original load is retried with recovered definitions. CSS invalidation clears this cache.
   * @throws If neither the original nor the recovered CSS graph can be resolved, read, and loaded.
   */
  async #editorDesignSystem(): Promise<EditorCssGraph> {
    if (!this.#editorCss) {
      if (this.#loader) {
        this.#editorCss = {
          loader: this.#loader,
          resources: [],
          lintSources: this.#lintSources,
          recovered: false,
          plugins: this.#plugins,
        };
      } else {
        this.#editorCss = await loadEditorCssGraph(
          path.resolve(this.host.root, cssEntrypoint(this.host.config)),
          compilerOptions(this.host.config),
          this.host.css,
          this.#pluginInputs(),
        );
      }
    }
    return this.#editorCss;
  }

  /**
   * Lazily selects the custom backend or creates the Node checker with fixed project settings.
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
 * Compares file paths as the host filesystem does.
 * @param a - File path resolved against the process working directory when relative.
 * @param b - File path resolved in the same way.
 * @returns Whether both paths name the same file, ignoring case on Windows.
 */
export function sameFile(a: string, b: string): boolean {
  /**
   * Normalizes a path for comparison.
   * @param file - File path.
   * @returns The absolute path, lowercased on Windows.
   */
  const key = (file: string) =>
    process.platform === "win32" ? path.resolve(file).toLowerCase() : path.resolve(file);
  return key(a) === key(b);
}

/**
 * Creates a result that carries no reports or graph after a capture becomes unusable.
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
