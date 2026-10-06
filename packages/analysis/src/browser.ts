import type { AnalysisProjectConfig } from "@stillwind/config";
import { BrowserProject, virtualPath } from "@stillwind/host/browser";
import { minimatch } from "minimatch";
import { relative, resolve } from "pathe";

import type {
  AnalysisWorkspaceService,
  EditorRequest,
  WorkspaceEditorResult,
  WorkspaceResult,
} from "./index.ts";

/** Browser workspace with original URIs and a complete virtual dependency inventory. */
export interface BrowserWorkspaceOptions {
  /** Configuration URI used to identify diagnostic groups. */
  config: string;
  /** Evaluated projects whose roots are absolute virtual paths. */
  projects: readonly AnalysisProjectConfig[];
  /** Saved source and dependency files, keyed by absolute virtual path. */
  files: ReadonlyMap<string, string>;
  /** Maps virtual paths back to workspace URIs. Defaults to file URIs. */
  documentUri?(this: void, path: string): string;
}

/** Shares snapshot compilation between LSP analysis and application builds. */
export class BrowserAnalysisWorkspace implements AnalysisWorkspaceService {
  readonly #options: BrowserWorkspaceOptions;
  readonly #documents = new Map<
    string,
    {
      source: string;
      version: number;
    }
  >();

  #revision = 0;
  #projects?: BrowserProject[];
  #buildRevision = -1;
  #closed = false;
  #queue: Promise<unknown> = Promise.resolve();

  async #prepareProjects(revision: number, files: ReadonlyMap<string, string>): Promise<void> {
    if (this.#buildRevision === revision) {
      return;
    }
    await Promise.all(this.#projects?.map((project) => project.close()) ?? []);
    this.#projects = this.#options.projects.map(
      (config) =>
        new BrowserProject({
          root: config.root!,
          config,
          files,
          documentUri: this.#options.documentUri,
        }),
    );
    this.#buildRevision = revision;
  }

  /** @param options - Evaluated settings and saved files. Create a new service when these change. */
  constructor(options: BrowserWorkspaceOptions) {
    this.#options = options;
  }

  /** Workspace revision increases after each effective buffer update or closure. */
  get revision(): number {
    return this.#revision;
  }

  /** @inheritdoc */
  update(uri: string, source: string, version: number): void {
    if (this.#closed) {
      throw new Error("Browser workspace is closed");
    }

    const id = virtualPath(uri);

    const previous = this.#documents.get(id);
    if (previous?.version === version && previous.source === source) {
      return;
    }
    if (!Number.isSafeInteger(version) || (previous && version <= previous.version)) {
      throw new Error("Analysis document versions must increase");
    }

    this.#documents.set(id, {
      source,
      version,
    });
    this.#revision++;
  }

  /** @inheritdoc */
  closeDocument(uri: string): void {
    if (this.#documents.delete(virtualPath(uri))) {
      this.#revision++;
    }
  }

  /** Returns a captured file inventory with open buffers applied. */
  files(): ReadonlyMap<string, string> {
    return new Map([
      ...this.#options.files,
      ...[...this.#documents].map(([id, document]) => [id, document.source] as const),
    ]);
  }

  /** @inheritdoc */
  analyze(uri: string, signal?: AbortSignal): Promise<WorkspaceResult[]> {
    const revision = this.#revision;
    const files = this.files();
    const id = virtualPath(uri);

    const task = this.#queue.then(async () => {
      if (this.#closed) {
        throw new Error("Browser workspace is closed");
      }

      const obsolete = () =>
        signal?.aborted
          ? ("cancelled" as const)
          : revision !== this.#revision
            ? ("stale" as const)
            : undefined;

      const results: WorkspaceResult[] = [];
      for (const [index, config] of this.#options.projects.entries()) {
        if (!includes(config, id)) {
          continue;
        }

        const project = new BrowserProject({
          root: config.root!,
          config,
          files,
          documentUri: this.#options.documentUri,
        });
        let result: WorkspaceResult["result"] = {
          entry: uri,
          revision,
          status: "incomplete",
          reports: [],
        };
        try {
          if (!obsolete()) {
            const graph = await project.analyze(id);
            result = {
              ...result,
              status:
                graph.unverified.length ||
                graph.dependencies.some(
                  (dependency) =>
                    dependency.status === "unresolved" || dependency.status === "unsupported",
                )
                  ? "incomplete"
                  : "complete",
              reports: graph.batches
                .flatMap((batch) => batch.reports)
                .filter((report) => includes(config, virtualPath(report.primary.location.uri))),
              dependencies: graph.dependencies,
              unverified: graph.unverified,
              generation: graph.generation,
            };
          }
        } catch (error) {
          result.error = error instanceof Error ? error.message : String(error);
        } finally {
          await project.close();
        }
        const status = obsolete();
        if (status) {
          result = {
            entry: uri,
            revision,
            status,
            reports: [],
          };
        }
        results.push({
          project: index,
          config: this.#options.config,
          result,
        });
      }
      return results;
    });

    this.#queue = task.catch(() => {});

    return task;
  }

  /**
   * Transforms a module for a browser application build.
   * @param uri - Source URI in exactly one configured project.
   * @param revision - Workspace revision captured when the application build started.
   * @returns Transformed source, CSS, reports, and generated declarations.
   * Blocked graphs retain diagnostics and return empty CSS and CSS values.
   * @throws If project selection fails or the build revision became obsolete.
   */
  compile(
    uri: string,
    revision: number,
  ): Promise<{
    graph: Awaited<ReturnType<BrowserProject["analyze"]>>;
    css: string;
    declarations: [string, string][];
    cssValues: [string, { css: string; parts: { key: string; css: string }[] }][];
  }> {
    const files = this.files();
    const id = virtualPath(uri);

    const task = this.#queue.then(async () => {
      if (this.#closed || revision !== this.#revision) {
        throw new Error("Browser build snapshot is obsolete");
      }

      const matches = this.#options.projects
        .map((config, index) => ({ config, index }))
        .filter(({ config }) => includes(config, id));
      if (matches.length !== 1) {
        throw new Error(`Expected one Stillwind project for ${uri}`);
      }

      await this.#prepareProjects(revision, files);

      const project = this.#projects![matches[0].index];
      const graph = await project.analyze(id);
      if (revision !== this.#revision) {
        throw new Error("Browser build snapshot is obsolete");
      }

      return {
        graph,
        css: graph.generation === "complete" ? project.css() : "",
        declarations: [...project.declarations()],
        cssValues: graph.generation === "complete" ? [...project.cssValues()] : [],
      };
    });

    this.#queue = task.catch(() => {});

    return task;
  }

  /** @inheritdoc */
  query(
    uri: string,
    request: EditorRequest,
    signal?: AbortSignal,
  ): Promise<WorkspaceEditorResult[]> {
    const revision = this.#revision;
    const files = this.files();
    const id = virtualPath(uri);
    const task = this.#queue.then(async () => {
      const obsolete = () => this.#closed || signal?.aborted || revision !== this.#revision;
      if (obsolete()) {
        return [];
      }
      await this.#prepareProjects(revision, files);
      const results: WorkspaceEditorResult[] = [];
      for (const [index, config] of this.#options.projects.entries()) {
        if (!includes(config, id)) {
          continue;
        }
        const result = await this.#projects![index].query(id, request);
        if (obsolete()) {
          return [];
        }
        if (result) {
          results.push({ project: index, config: this.#options.config, result });
        }
      }
      return results;
    });
    this.#queue = task.catch(() => {});
    return task;
  }

  /** Releases retained build sessions and waits for queued analyses. */
  async close(): Promise<void> {
    this.#closed = true;
    this.#revision++;
    await this.#queue;
    await Promise.all(this.#projects?.map((project) => project.close()) ?? []);
  }
}

function includes(config: AnalysisProjectConfig, id: string): boolean {
  const name = relative(resolve(config.root!), id);

  return (
    !name.startsWith("../") &&
    !name.split("/").includes("node_modules") &&
    config.files.some((pattern) => minimatch(name, pattern)) &&
    !(config.exclude ?? []).some((pattern) => minimatch(name, pattern))
  );
}
