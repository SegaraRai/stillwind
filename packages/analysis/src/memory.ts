import type { AnalysisResult, AnalysisWorkspaceService, WorkspaceResult } from "./index.ts";

/** Compiler input captured from URI documents before asynchronous compilation starts. */
export interface MemoryAnalysisInput {
  /** Requested target URI accepted by the embedding's includes callback. */
  uri: string;
  /** Workspace change counter captured when the request was queued. */
  revision: number;
  /** Original text of all open documents, keyed by URI. The snapshot is independent of later updates. */
  documents: ReadonlyMap<string, string>;
  /** Optional cancellation signal. The workspace also checks it before and after calling compile. */
  signal?: AbortSignal;
}

/** Supplies the browser's WASM compiler, module loader, and type-facts service. */
export interface MemoryAnalysisOptions {
  /** Identifies the project in diagnostics. It need not be a filesystem path. */
  config: string;
  /**
   * Tests whether this workspace owns a target. Dependency buffers can be outside this scope.
   *
   * @param uri - Requested target URI.
   * @returns Whether compile should run for this target.
   */
  includes(uri: string): boolean;
  /**
   * Runs the embedding's compiler against one captured snapshot.
   *
   * @param input - Target, revision, open buffers, and optional cancellation signal.
   * @returns Reports and coverage information for the captured revision. Complete describes analysis
   * coverage. It does not imply that every report is successful.
   * @throws If compiler or module services fail. The workspace returns the error as an incomplete result.
   */
  compile(input: MemoryAnalysisInput): Promise<
    Pick<AnalysisResult, "reports" | "dependencies" | "unverified" | "generation" | "error"> & {
      status: "complete" | "incomplete";
    }
  >;
  /**
   * Releases resources after queued compilations finish. Called once during workspace closure.
   *
   * @returns Nothing, or a promise for asynchronous cleanup.
   * @throws If resource cleanup fails. The workspace's close promise rejects.
   */
  close?(): void | Promise<void>;
}

/**
 * Serializes in-memory analyses and discards reports from obsolete document revisions.
 *
 * @param options - Project membership, compiler operation, and optional resource cleanup.
 * @returns A URI-based service. It supplies snapshots to compile but performs no module resolution
 * itself. Closing waits for queued work and calls the cleanup operation once.
 */
export function createMemoryAnalysisWorkspace(
  options: MemoryAnalysisOptions,
): AnalysisWorkspaceService {
  const documents = new Map<string, { source: string; version: number }>();

  let revision = 0;
  let queue: Promise<unknown> = Promise.resolve();
  let closed = false;
  let closing: Promise<void> | undefined;

  return {
    update(uri, source, version) {
      if (closed) {
        throw new Error("Analysis workspace is closed");
      }
      const previous = documents.get(uri);
      if (previous?.version === version && previous.source === source) {
        return;
      }
      if (!Number.isSafeInteger(version) || (previous && previous.version >= version)) {
        throw new Error("Analysis document versions must increase");
      }
      documents.set(uri, { source, version });
      revision++;
    },
    closeDocument(uri) {
      if (documents.delete(uri)) {
        revision++;
      }
    },
    analyze(uri, signal): Promise<WorkspaceResult[]> {
      if (closed) {
        return Promise.reject(new Error("Analysis workspace is closed"));
      }
      if (!options.includes(uri)) {
        return Promise.resolve([]);
      }
      const captured = revision;
      const snapshot = new Map([...documents].map(([uri, doc]) => [uri, doc.source]));
      const task = queue.then(async () => {
        const obsolete = () =>
          signal?.aborted
            ? ("cancelled" as const)
            : captured !== revision
              ? ("stale" as const)
              : undefined;
        let result: AnalysisResult = {
          entry: uri,
          revision: captured,
          status: "incomplete",
          reports: [],
        };
        if (!obsolete()) {
          try {
            result = {
              ...result,
              ...(await options.compile({
                uri,
                revision: captured,
                documents: snapshot,
                signal,
              })),
            };
          } catch (error) {
            result.error = String(error);
          }
        }
        const status = obsolete();
        if (status) {
          result = {
            entry: uri,
            revision: captured,
            status,
            reports: [],
          };
        }
        return [
          {
            project: 0,
            config: options.config,
            result,
          },
        ];
      });
      queue = task.catch(() => {});
      return task;
    },
    close() {
      closing ??= (async () => {
        closed = true;
        revision++;
        await queue;
        await options.close?.();
      })();
      return closing;
    },
  };
}
