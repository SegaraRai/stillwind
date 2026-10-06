import type { AnalysisResult, AnalysisWorkspaceService, WorkspaceResult } from "./index.ts";

/** Compiler input captured from URI documents before asynchronous compilation starts. */
export interface MemoryAnalysisInput {
  uri: string;
  revision: number;
  documents: ReadonlyMap<string, string>;
  signal?: AbortSignal;
}

/** Supplies the browser's WASM compiler, module loader, and type-facts service. */
export interface MemoryAnalysisOptions {
  /** Identifies the project in diagnostics. It need not be a filesystem path. */
  config: string;
  includes(uri: string): boolean;
  compile(input: MemoryAnalysisInput): Promise<
    Pick<AnalysisResult, "reports" | "dependencies" | "unverified" | "generation" | "error"> & {
      status: "complete" | "incomplete";
    }
  >;
  /** Runs after pending compilations finish, releasing resources owned by the compiler. */
  close?(): void | Promise<void>;
}

/** Serializes in-memory analyses and suppresses results from obsolete document revisions. */
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
