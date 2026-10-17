import { captureProjectConfig, type AnalysisProjectConfig } from "@stillwind/config";
import {
  BrowserProject,
  virtualPath,
  type BrowserModuleCss,
  type BrowserStylesheetOutput,
  type EditorRequest,
} from "@stillwind/host/browser";
import { minimatch } from "minimatch";
import { relative, resolve } from "pathe";

import type {
  AnalysisWorkspaceService,
  WorkspaceEditorResult,
  WorkspaceResult,
} from "./workspace.ts";

/** Browser workspace with original URIs and a complete virtual dependency inventory. */
export interface BrowserWorkspaceOptions {
  /** Configuration URI used to identify diagnostic groups. */
  config: string;
  /** Evaluated projects whose roots are absolute virtual paths. */
  projects: readonly AnalysisProjectConfig[];
  /** Saved source and dependency files, keyed by absolute virtual path. */
  files: ReadonlyMap<string, string>;
  /**
   * Maps virtual paths back to workspace URIs. Defaults to file URIs.
   *
   * @param path - Decoded absolute virtual path.
   * @returns Original URI used in diagnostics and source maps.
   */
  documentUri?(this: void, path: string): string;
}

/** Shares snapshot compilation between LSP analysis and application builds. */
export class BrowserAnalysisWorkspace implements AnalysisWorkspaceService {
  /** Owned settings and saved inventory fixed when this service is created. */
  readonly #options: BrowserWorkspaceOptions;
  /** Open buffers keyed by decoded virtual path, applied over the saved inventory. */
  readonly #documents = new Map<
    string,
    {
      /** Complete original text of the open document. */
      source: string;
      /** Last accepted safe integer version for this open document. */
      version: number;
    }
  >();

  /** Change counter captured by analyses, editor queries, and application builds. */
  #revision = 0;
  /** Retained projects shared by editor queries and builds for one inventory revision. */
  #projects?: BrowserProject[];
  /** Revision represented by the retained projects. -1 means they have not been prepared. */
  #buildRevision = -1;
  /** Prevents new analysis and invalidates queued editor queries during closure. */
  #closed = false;
  /** Serializes compiler operations and absorbs failures for subsequent operations. */
  #queue: Promise<unknown> = Promise.resolve();

  /**
   * Replaces retained projects when their inventory revision changes.
   *
   * @param revision - Revision captured with the supplied files by the queued caller.
   * @param files - Complete saved inventory with captured open buffers applied.
   * @returns Completion of old project closure and new project creation, or no change for the same revision.
   * @throws If a retained project fails to close or a replacement cannot be constructed.
   */
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

  /**
   * Creates a service without compiling its projects yet.
   * @param options - Evaluated settings and saved files captured by this service.
   * Replace the service to apply new configuration or saved inventory. Open buffers use update.
   */
  constructor(options: BrowserWorkspaceOptions) {
    this.#options = {
      ...options,
      projects: options.projects.map(captureProjectConfig),
      files: new Map(options.files),
    };
  }

  /**
   * Reads the revision required when requesting an application build.
   * @returns Change counter that increases after an effective buffer update or closure.
   */
  get revision(): number {
    return this.#revision;
  }

  /**
   * Stores an original buffer and increments the workspace revision when it changes.
   * @param uri - Original document URI or virtual path, including dependency files outside target scopes.
   * @param source - Complete unsaved text.
   * @param version - Safe integer greater than the previous version. Identical text at the same version is accepted.
   * @throws If the service is closed or the version is invalid or does not increase.
   */
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

  /**
   * Removes an open buffer and increments the revision only when that buffer existed.
   * @param uri - Original URI or virtual path. Future snapshots use saved text for this path.
   */
  closeDocument(uri: string): void {
    if (this.#documents.delete(virtualPath(uri))) {
      this.#revision++;
    }
  }

  /**
   * Captures saved files with open buffers applied.
   * @returns An independent map keyed by absolute virtual path. Later buffer changes do not alter it.
   */
  files(): ReadonlyMap<string, string> {
    return new Map([
      ...this.#options.files,
      ...[...this.#documents].map(([id, document]) => [id, document.source] as const),
    ]);
  }

  /**
   * Serializes analysis of a captured inventory using a temporary project for each matching scope.
   * A stylesheet that a project lints matches that project, and its reports are its stylesheet
   * lint findings.
   * @param uri - Original target URI or virtual path.
   * @param signal - Optional cancellation checked before and after compiler work.
   * @returns Results in project order. Compiler failures produce incomplete results, and obsolete
   * or cancelled results omit reports. Targets outside all scopes return an empty list.
   * @throws If the service is closed or a temporary project cannot be created or closed.
   */
  analyze(uri: string, signal?: AbortSignal): Promise<WorkspaceResult[]> {
    const revision = this.#revision;
    const files = this.files();
    const id = virtualPath(uri);

    const task = this.#queue.then(async () => {
      if (this.#closed) {
        throw new Error("Browser workspace is closed");
      }

      /**
       * Classifies work that can no longer publish results from the captured revision.
       * @returns `cancelled` for an aborted signal, `stale` for a newer revision, or undefined.
       */
      const obsolete = () =>
        signal?.aborted
          ? ("cancelled" as const)
          : revision !== this.#revision
            ? ("stale" as const)
            : undefined;

      const results: WorkspaceResult[] = [];
      const stylesheet = isStylesheet(id);
      for (const [index, config] of this.#options.projects.entries()) {
        if (!includes(config, id) && !stylesheet) {
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
          if (stylesheet) {
            const lint = await project.stylesheetLint().catch(() => undefined);
            if (!lint?.sources.includes(id)) {
              continue;
            }
            result = {
              ...result,
              status: "complete",
              reports: lint.reports.filter(
                (report) => virtualPath(report.primary.location.uri) === id,
              ),
            };
          } else if (!obsolete()) {
            const graph = await project.analyze(id);
            const observations = structuredClone({
              reports: graph.batches
                .flatMap((batch) => batch.reports)
                .filter((report) => includes(config, virtualPath(report.primary.location.uri))),
              dependencies: graph.dependencies,
              unverified: graph.unverified,
            }) as Pick<WorkspaceResult["result"], "reports" | "dependencies" | "unverified">;
            result = {
              ...result,
              ...observations,
              status:
                graph.unverified.length ||
                graph.dependencies.some(
                  (dependency) =>
                    dependency.status === "unresolved" || dependency.status === "unsupported",
                )
                  ? "incomplete"
                  : "complete",
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
   * @returns Transformed source, CSS, and reports.
   * Blocked graphs retain diagnostics and return empty CSS and CSS values.
   * @throws If project selection fails or the build revision became obsolete.
   */
  compile(
    uri: string,
    revision: number,
  ): Promise<{
    /** Transformed source graph, including diagnostic reports and generation eligibility. */
    graph: Awaited<ReturnType<BrowserProject["analyze"]>>;
    /** Current combined CSS, empty when the graph blocks generation. */
    css: string;
    /** Naming fallbacks observed during document assembly. */
    rejectedNames: BrowserStylesheetOutput["rejectedNames"];
    /** Module identity and its CSS record, empty when generation is blocked. */
    cssValues: [string, BrowserModuleCss][];
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

      const stylesheet =
        graph.generation === "complete"
          ? project.css()
          : {
              css: "",
              rejectedNames: [],
            };
      return {
        graph,
        ...stylesheet,
        cssValues: graph.generation === "complete" ? [...project.cssValues()] : [],
      };
    });

    this.#queue = task.catch(() => {});

    return task;
  }

  /**
   * Queries retained projects against a captured inventory and discards obsolete answers.
   * @param uri - Original target URI or virtual path.
   * @param request - Operation with offsets in the original UTF-16 document.
   * @param signal - Optional cancellation checked around project preparation and queries.
   * @returns Answers in project order, or an empty list after closure, cancellation, or a revision change.
   * @throws If project preparation or an editor query fails.
   */
  query(
    uri: string,
    request: EditorRequest,
    signal?: AbortSignal,
  ): Promise<WorkspaceEditorResult[]> {
    const revision = this.#revision;
    const files = this.files();
    const id = virtualPath(uri);
    const task = this.#queue.then(async () => {
      /**
       * Checks whether this query must discard its captured results.
       * @returns Whether the service is closed, the signal is aborted, or the revision has changed.
       */
      const obsolete = () => this.#closed || signal?.aborted || revision !== this.#revision;
      if (obsolete()) {
        return [];
      }
      await this.#prepareProjects(revision, files);
      const results: WorkspaceEditorResult[] = [];
      for (const [index, config] of this.#options.projects.entries()) {
        if (!includes(config, id) && !isStylesheet(id)) {
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

  /**
   * Marks the service closed, invalidates captured revisions, and releases retained projects after queued work.
   * @returns Completion of queued work and project cleanup.
   * @throws If a retained project fails to close.
   */
  async close(): Promise<void> {
    this.#closed = true;
    this.#revision++;
    await this.#queue;
    await Promise.all(this.#projects?.map((project) => project.close()) ?? []);
  }
}

/**
 * Checks whether a virtual identity names a stylesheet.
 * @param id - Absolute virtual file path.
 * @returns Whether the complete identity ends in `.css`, ignoring case.
 */
function isStylesheet(id: string): boolean {
  return /\.css$/i.test(id);
}

/**
 * Checks a virtual path against a project's target globs and exclusions.
 * @param config - Evaluated configuration with an absolute root.
 * @param id - Absolute virtual file path.
 * @returns Whether the path is under the root, outside node_modules, included, and not excluded.
 */
function includes(config: AnalysisProjectConfig, id: string): boolean {
  const name = relative(resolve(config.root!), id);

  return (
    !name.startsWith("../") &&
    !name.split("/").includes("node_modules") &&
    config.files.some((pattern) => minimatch(name, pattern)) &&
    !(config.exclude ?? []).some((pattern) => minimatch(name, pattern))
  );
}
