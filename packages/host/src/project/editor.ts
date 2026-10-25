import {
  NativeProjectSession,
  type BindingOutput,
  type NativeDesignSystemLoader,
} from "@stillwind/native";

import { cleanModuleId, compileModule, type CompilerHost } from "../compilation/index.ts";
import type { LoadedCssGraph } from "../css/css.ts";
import { attachPlugins } from "../css/plugins/session.ts";
import {
  TypeScriptFactsService,
  resolveTypeFactsEnvironment,
  type TypeFactsEnvironment,
  type TypeScriptFactsStats,
} from "../semantics/index.ts";

/** Versioned source buffer that can replace a filesystem read during editor analysis. */
export interface EditorDocument {
  /**
   * Resolver identity of the document. Editor operations remove query and fragment suffixes for
   * ordinary file IDs.
   */
  id: string;
  /**
   * Safe integer, including negative values, that must increase for each update while this
   * document stays open.
   */
  version: number;
  /** Current unsaved document text supplied to source preparation and compilation. */
  source: string;
}

/**
 * Result of analyzing one document against a captured editor revision, with any previously
 * successful preview.
 */
export interface EditorAnalysis {
  /** Copy of the entry buffer captured when analysis started. */
  document: EditorDocument;
  /**
   * Project change counter captured at analysis start. Any document update, close of an open
   * document, or invalidation advances it.
   */
  revision: number;
  /** Observed coverage: the result includes only dependencies demanded by this analysis. */
  coverage: "observed";
  /**
   * Success means usable code without compiler errors or deny-level lint. Partial retains an
   * incomplete or rejected result. Stale means the project revision changed. Cancelled means the
   * signal was aborted. Stale and cancelled results omit output.
   */
  status: "success" | "partial" | "stale" | "cancelled";
  /**
   * Current compilation output, including reports for partial results when available. Stale or
   * cancelled analysis removes this field.
   */
  output?: BindingOutput;
  /**
   * Caught exception message. Compiler reports returned in collect mode do not populate this field.
   */
  error?: string;
  /**
   * Distinct dependency resolver identities observed by the compilation, in first observation
   * order, including those observed before failure.
   */
  dependencies: string[];
  /**
   * Copy of checker counters when this analysis needed type facts. Omitted when no checker was
   * created.
   */
  checkerStats?: TypeScriptFactsStats;
  /**
   * Successful preview retained for this document when analysis started. Failed, stale, or
   * cancelled attempts do not replace the stored success.
   */
  previousSuccess?: {
    /** Document version that produced the retained preview. */
    version: number;
    /** Project revision captured by the successful analysis. */
    revision: number;
    /** Successful compiler output retained for editor preview. */
    output: BindingOutput;
  };
}

/** Versioned, unpublished previews. Each request owns its compiler session. */
export class EditorProject {
  /**
   * Open unsaved buffers keyed by cleaned resolver identity. Analysis captures a map of these
   * buffers before asynchronous work.
   */
  readonly #documents = new Map<string, EditorDocument>();
  /**
   * Most recent successful output for each cleaned document identity, retained across later
   * unsuccessful attempts.
   */
  readonly #successes = new Map<string, NonNullable<EditorAnalysis["previousSuccess"]>>();
  /**
   * Project change counter used to reject results whose source or dependencies changed during
   * analysis.
   */
  #revision = 0;

  /** Filesystem root used to initialize the type-facts service when an analysis needs it. */
  readonly root: string;
  /**
   * Dependency resolver and source metadata operations. Open buffers override its reader during
   * analysis.
   */
  readonly host: CompilerHost;
  /**
   * Factory that must return a completed design-system loader suitable for a new compiler session
   * on every analysis, or a loaded CSS graph whose plugins define dynamic candidates.
   */
  readonly designSystem: () =>
    | NativeDesignSystemLoader
    | Pick<LoadedCssGraph, "loader" | "plugins">;
  /** Fixed checker settings reused by independent analyses and retained without rereading tsconfig. */
  readonly #typeFactsEnvironment: TypeFactsEnvironment;

  /**
   * Creates an editor project without opening documents or creating a checker.
   *
   * @param root - Filesystem project root used for checker resolution.
   * @param host - Source descriptors, dependency resolver, and reader used by analysis.
   * @param designSystem - Factory returning a loaded design system, or a loaded CSS graph with its
   * plugins, for each independent analysis.
   * @param typeFactsEnvironment - Fixed checker settings whose root matches this project. Omission
   * selects an existing root tsconfig or isolated defaults.
   */
  constructor(
    root: string,
    host: CompilerHost,
    designSystem: () => NativeDesignSystemLoader | Pick<LoadedCssGraph, "loader" | "plugins">,
    typeFactsEnvironment = resolveTypeFactsEnvironment(root),
  ) {
    this.root = root;
    this.host = host;
    this.designSystem = designSystem;
    this.#typeFactsEnvironment = typeFactsEnvironment;
  }

  /**
   * Stores an unsaved buffer and advances the project revision.
   *
   * @param document - Buffer with a safe integer version greater than the current version for its
   * cleaned identity. A copy is stored.
   * @throws If the version is not a safe integer or does not increase for an open document.
   */
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

  /**
   * Removes an open buffer and its retained successful preview.
   *
   * @param id - Document identity, cleaned in the same way as `update`. Removing an open buffer
   * advances the revision. An unknown identity has no effect.
   */
  close(id: string): void {
    id = cleanModuleId(id);
    if (this.#documents.delete(id)) {
      this.#revision += 1;
    }
    this.#successes.delete(id);
  }

  /**
   * Advances the revision after disk dependencies or the design system change, making in-progress
   * analysis stale.
   */
  invalidate(): void {
    this.#revision += 1;
  }

  /**
   * Compiles an open document with the unsaved buffers captured at analysis start.
   *
   * The analysis owns a new compiler session and closes any checker it creates. Dependency reads
   * prefer captured open buffers. Compiler diagnostics are collected. Other failures become the
   * returned error text. Stale or cancelled work retains neither current output nor a new
   * successful preview.
   *
   * @param id - Identity of an open document. Ordinary query and fragment suffixes are removed.
   * @param signal - Optional cancellation signal checked around asynchronous dependency operations
   * and before returning.
   * @returns A promise for the versioned result, observed dependencies, optional checker counters,
   * and the success retained when analysis started.
   * @throws If the document is not open. Cleanup failures can also reject the promise.
   */
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
    /**
     * Requires this analysis to retain the captured project revision and an active signal.
     * @throws If cancellation or a newer project revision makes this analysis obsolete.
     */
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
      const design = this.designSystem();
      const graph =
        "loader" in design
          ? design
          : {
              loader: design,
              plugins: undefined,
            };
      const session = new NativeProjectSession(graph.loader, false);
      attachPlugins(session, graph.plugins);
      const output = await compileModule(
        session,
        () => {
          facts ??= new TypeScriptFactsService(this.#typeFactsEnvironment);
          return facts;
        },
        document.source,
        id,
        {
          describe: (dependency) => this.host.describe(dependency),
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
        {
          diagnostics: "collect",
          typeFactsInputs: {
            overlayFiles: new Map([...overlays].map(([id, value]) => [id, value.source])),
          },
        },
      );
      assertActive();
      result.output = output;
      const denied = output?.lintFindings.some(
        (finding) => output.lintSettings[finding.rule] === "deny",
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
        result.checkerStats = {
          ...facts.stats,
        };
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
