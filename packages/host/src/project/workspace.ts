import path from "node:path";
import { fileURLToPath } from "node:url";

import { captureProjectConfig, type CapturedProjectConfig } from "@stillwind/config";
import { loadConfiguration } from "@stillwind/config/node";
import { minimatch } from "minimatch";
import { glob } from "tinyglobby";

import { sourceKey } from "../compilation/project.ts";
import { isStylesheet } from "../css/index.ts";
import type { EditorRequest, EditorResult } from "../editor/index.ts";
import { sameFile } from "./analysis/host.ts";
import { AnalysisProject, type AnalysisResult } from "./analysis/project.ts";
import { createFileSystemAnalysisHost } from "./filesystem.ts";

/** A project's configuration and results, retained independently when scopes overlap. */
export interface WorkspaceProject {
  /** Evaluated project settings. Relative root and scope globs use configFile's directory and host root. */
  readonly config: CapturedProjectConfig;
  /** Absolute path of the configuration that owns this project. */
  readonly configFile: string;
  /** Owned analysis state. Buffers outside this project's target scope can still be dependencies. */
  readonly analysis: AnalysisProject;
}

/** Analysis results for one explicitly enumerated target in one project. */
export interface WorkspaceResult {
  /** Zero-based index into the workspace's projects array. */
  project: number;
  /** Absolute configuration path used to group this result. */
  config: string;
  /** Captured result whose reports have been filtered to this project's configured scope. */
  result: AnalysisResult;
}

/** Owns every project in one evaluated configuration and keeps overlapping scopes independent. */
export class AnalysisWorkspace {
  /** Absolute paths with open buffers, including paths outside the configured target globs. */
  readonly #openDocuments = new Set<string>();
  /** Configuration files and imports whose changes require reopening this workspace. */
  readonly #configurationDependencies: readonly string[];
  /** Absolute path evaluated when this workspace opened. Reopen the workspace to apply configuration changes. */
  readonly configFile: string;
  /** Fixed project records in configuration order. Each analysis service owns mutable state. */
  readonly projects: readonly WorkspaceProject[];

  /**
   * Retains the evaluated configuration and takes ownership of its initialized projects.
   *
   * @param configFile - Absolute configuration path.
   * @param projects - Successfully opened hosts in configuration order.
   * @param dependencies - Absolute configuration and import paths recorded during evaluation.
   */
  private constructor(
    configFile: string,
    projects: WorkspaceProject[],
    dependencies: readonly string[],
  ) {
    this.configFile = configFile;
    this.projects = projects;
    this.#configurationDependencies = dependencies;
  }

  /**
   * Evaluates configuration and opens every project, closing earlier hosts if initialization fails.
   *
   * @param configFile - Trusted configuration path. Relative paths use the process working directory.
   * @returns A workspace that owns its project hosts. Close it when replacing the configuration.
   * @throws If configuration evaluation or project initialization fails.
   */
  static async open(configFile: string): Promise<AnalysisWorkspace> {
    configFile = path.resolve(configFile);
    const { config, dependencies } = await loadConfiguration(configFile);
    const projects: WorkspaceProject[] = [];
    try {
      for (const supplied of config.projects) {
        const project = captureProjectConfig(supplied);
        const host = createFileSystemAnalysisHost(project, path.dirname(configFile));
        projects.push({
          config: project,
          configFile,
          analysis: new AnalysisProject(host),
        });
      }
      return new AnalysisWorkspace(configFile, projects, dependencies);
    } catch (error) {
      await Promise.allSettled(projects.map((project) => project.analysis.close()));
      throw error;
    }
  }

  /**
   * Matches target scope independently of whether the source exists on disk.
   *
   * @param project - Project whose root, include globs, and exclusions define membership.
   * @param file - Absolute file path to test.
   * @returns Whether the path matches files without matching excludes, node_modules, or .git.
   * Dependency traversal does not use this filter.
   */
  includes(project: WorkspaceProject, file: string): boolean {
    const name = path.relative(project.analysis.host.root, file).replaceAll("\\", "/");
    /**
     * Matches this project-relative path, including dotfiles.
     * @param pattern - Include or exclude glob interpreted by minimatch.
     * @returns Whether the normalized path matches the pattern.
     */
    const matches = (pattern: string) =>
      minimatch(name, pattern, {
        dot: true,
      });
    return (
      project.config.files.some(matches) &&
      !["**/node_modules/**", "**/.git/**", ...(project.config.exclude ?? [])].some(matches)
    );
  }

  /**
   * Checks whether the project lints a stylesheet. Loading the project's CSS graph decides it.
   *
   * @param project - Project whose CSS graph lists its linted stylesheets.
   * @param file - Absolute file path to test.
   * @param editor - When false (the default), requires the original CSS graph to load successfully.
   * When true, retries a failed load with recovered definitions so completion, hover, and colors
   * can still identify stylesheet membership while the CSS is incomplete.
   * @returns Whether the file is the CSS entry or an imported file that the CSS host marks for
   * lint. Returns false for other extensions or when the selected loading mode fails. Ordinary
   * script analysis reports CSS loading failures.
   */
  async lintsStylesheet(project: WorkspaceProject, file: string, editor = false): Promise<boolean> {
    if (!isStylesheet(file)) {
      return false;
    }
    const sources = await project.analysis.stylesheets(editor).catch(() => []);
    return sources.some((source) => sameFile(source, file));
  }

  /**
   * Enumerates existing target files independently of dependency traversal.
   *
   * @param project - Project whose files and excludes globs are relative to its host root.
   * @param patterns - Optional additional include globs relative to the same root. An empty list
   * adds no filtering. These patterns cannot expand the configured scope.
   * @param options - Target kinds to add to the configured script targets.
   * @returns Sorted absolute target paths. Unsaved files absent from disk are not enumerated.
   * @throws If filesystem enumeration fails.
   */
  async targets(
    project: WorkspaceProject,
    patterns?: string[],
    options: {
      /** Adds the stylesheets that the project lints, as {@link lintsStylesheet} selects them. */
      stylesheets?: boolean;
    } = {},
  ): Promise<string[]> {
    const filters = patterns?.map((pattern) => {
      if (path.isAbsolute(pattern)) {
        return {
          file: path.normalize(pattern),
        };
      }
      return {
        pattern: pattern.replace(/^(\.\/)+/, ""),
      };
    });
    const files = await glob(project.config.files, {
      cwd: project.analysis.host.root,
      absolute: true,
      onlyFiles: true,
      dot: true,
      ignore: ["**/node_modules/**", "**/.git/**", ...(project.config.exclude ?? [])],
    });
    if (options.stylesheets) {
      files.push(...(await project.analysis.stylesheets().catch(() => [])));
    }
    return files
      .filter(
        (file, index) =>
          files.findIndex((other) => sameFile(other, file)) === index &&
          (!filters?.length ||
            filters.some((filter) =>
              filter.file !== undefined
                ? path.normalize(file) === filter.file
                : minimatch(
                    path.relative(project.analysis.host.root, file).replaceAll("\\", "/"),
                    filter.pattern!,
                    {
                      dot: true,
                    },
                  ),
            )),
      )
      .sort();
  }

  /**
   * Keeps editor answers separate when several projects include the same target.
   *
   * @param file - Absolute target path used for independent scope checks.
   * @param request - Operation in the original document's UTF-16 coordinates.
   * @param signal - Optional cancellation forwarded to each project's queued query.
   * @returns Available answers in configuration order, including overlapping project memberships.
   * CSS membership checks allow recovered definitions when the original graph cannot load.
   * Plugin path completion and links also accept CSS files inside a project's root.
   * @throws If a project's editor query fails.
   */
  async query(
    file: string,
    request: EditorRequest,
    signal?: AbortSignal,
  ): Promise<
    {
      /** Zero-based index into the configured projects. */
      project: number;
      /** Absolute configuration path that owns this project. */
      config: string;
      /** Answer tied to the captured original source and design system. */
      result: EditorResult;
    }[]
  > {
    const results: {
      /** Zero-based index into the configured projects. */
      project: number;
      /** Absolute configuration path that owns this project. */
      config: string;
      /** Answer tied to the captured original source and design system. */
      result: EditorResult;
    }[] = [];
    for (const [index, project] of this.projects.entries()) {
      const relative = path.relative(project.analysis.host.root, file);
      const navigation =
        isStylesheet(file) &&
        ["completion", "resolve", "links"].includes(request.kind) &&
        relative !== ".." &&
        !relative.startsWith(`..${path.sep}`) &&
        !path.isAbsolute(relative);
      if (
        !this.includes(project, file) &&
        !navigation &&
        !(await this.lintsStylesheet(project, file, true))
      ) {
        continue;
      }
      const result = await project.analysis.query(file, request, signal);
      if (result) {
        results.push({
          project: index,
          config: project.configFile,
          result,
        });
      }
    }
    return results;
  }

  /**
   * Updates all hosts because an open file can be a dependency outside their target scope.
   *
   * @param file - File path resolved by each project from the process working directory if relative.
   * @param source - Complete unsaved document text.
   * @param version - Increasing safe integer for the open buffer. Identical repeated updates have no effect.
   * @throws If a project rejects the version. Updates already applied to earlier projects are retained.
   */
  update(file: string, source: string, version: number): void {
    for (const project of this.projects) {
      project.analysis.update(file, source, version);
    }
    this.#openDocuments.add(path.resolve(file));
  }

  /**
   * Removes a buffer from every project so later reads use its source provider.
   *
   * @param file - File path resolved in the same way as update. Unknown buffers have no effect.
   */
  closeDocument(file: string): void {
    this.#openDocuments.delete(path.resolve(file));
    for (const project of this.projects) {
      project.analysis.closeDocument(file);
    }
  }

  /**
   * Checks whether a change to a file requires a new workspace.
   *
   * @param file - Changed file path.
   * @param reads - Result of `AnalysisProject.reads` for each project, in project order.
   * @returns True for a configuration dependency, and for a tsconfig or package.json that a project
   * read or that lies directly in a project root. Module resolution can read these files without
   * recording them, so the project root is checked as well.
   */
  #reopens(file: string, reads: readonly ReadonlySet<string>[]): boolean {
    if (this.#configurationDependencies.some((dependency) => sameFile(dependency, file))) {
      return true;
    }
    return (
      /(?:^|[/\\])(?:tsconfig(?:\.[^/\\]+)?\.json|package\.json)$/.test(file) &&
      this.projects.some(
        (project, index) =>
          reads[index].has(sourceKey(file)) ||
          sameFile(path.dirname(path.resolve(file)), project.analysis.host.root),
      )
    );
  }

  /**
   * Checks whether the disk content of a file can change a project's analysis.
   *
   * @param index - Project index.
   * @param file - Changed file path.
   * @param reads - Result of `AnalysisProject.reads` for each project, in project order.
   * @returns True when the file has no open buffer and the project read it or its target scope
   * contains it. Scope membership covers a new file that no analysis has read.
   */
  #stale(index: number, file: string, reads: readonly ReadonlySet<string>[]): boolean {
    return (
      ![...this.#openDocuments].some((open) => sameFile(open, file)) &&
      (reads[index].has(sourceKey(file)) || this.includes(this.projects[index], path.resolve(file)))
    );
  }

  /**
   * Checks whether file changes can change this workspace's results, without changing any state.
   * Analysis cannot depend on a file that it never read and that lies outside every target scope.
   *
   * @param files - File paths reported by a watch notification. Relative paths use the working
   * directory.
   * @returns Whether `refresh` would request reopening or invalidate a project for these files.
   */
  affects(files: readonly string[]): boolean {
    const reads = this.projects.map((project) => project.analysis.reads());
    return files.some(
      (file) =>
        this.#reopens(file, reads) || this.projects.some((_, i) => this.#stale(i, file, reads)),
    );
  }

  /**
   * Checks whether save or watch notifications need a new workspace, and invalidates each project
   * that a change to a closed file can affect.
   *
   * @param files - File paths reported by the notification. Relative paths use the working directory.
   * @returns False when the workspace must reopen, which leaves projects unchanged. Otherwise
   * returns true. This method does not read disk or reopen projects.
   */
  refresh(files: readonly string[]): boolean {
    const reads = this.projects.map((project) => project.analysis.reads());
    if (files.some((file) => this.#reopens(file, reads))) {
      return false;
    }
    for (const [index, project] of this.projects.entries()) {
      if (files.some((file) => this.#stale(index, file, reads))) {
        project.analysis.invalidate();
      }
    }
    return true;
  }

  /**
   * Advances every project revision and requests CSS and resolver cache refresh before its next analysis.
   */
  invalidate(): void {
    for (const project of this.projects) {
      project.analysis.invalidate();
    }
  }

  /**
   * Analyzes a target separately in every matching project and filters reports to each scope.
   * A project matches a script target in its configured scope and a stylesheet that it lints.
   *
   * @param file - Absolute target path. The file need not exist on disk when an open buffer supplies it.
   * @param signal - Optional cancellation checked by each project's queued analysis.
   * @returns Results in project order, or an empty list when no project includes the target.
   * Dependency information remains available even when its reports lie outside the target scope.
   */
  async analyze(file: string, signal?: AbortSignal): Promise<WorkspaceResult[]> {
    const results: WorkspaceResult[] = [];
    for (const [index, project] of this.projects.entries()) {
      if (this.includes(project, file) || (await this.lintsStylesheet(project, file))) {
        results.push({
          project: index,
          config: project.configFile,
          result: this.scopedResult(project, await project.analysis.analyze(file, signal)),
        });
      }
    }
    return results;
  }

  /**
   * Analyzes every selected target, including files unreachable from an application entry, and
   * every stylesheet that a project lints.
   *
   * @param patterns - Optional additional globs relative to each project's root. Empty or omitted
   * filters check every configured target.
   * @returns Scoped results in project order, then sorted target order. Overlapping memberships remain separate.
   * @throws If target enumeration fails. Analysis failures are represented in individual results.
   */
  async lint(patterns?: string[]): Promise<WorkspaceResult[]> {
    const results: WorkspaceResult[] = [];
    for (const [index, project] of this.projects.entries()) {
      const files = await this.targets(project, patterns, { stylesheets: true });
      await project.analysis.prepare(files);
      for (const file of files) {
        results.push({
          project: index,
          config: project.configFile,
          result: this.scopedResult(project, await project.analysis.analyze(file)),
        });
      }
    }
    return results;
  }

  /**
   * Prepares targets together in each project that includes them.
   *
   * @param files - Absolute target paths. Files outside a project's target globs are skipped.
   * @returns Completion of preparation. Each target's analysis reports its own source failures.
   */
  async prepare(files: readonly string[]): Promise<void> {
    for (const project of this.projects) {
      await project.analysis.prepare(files.filter((file) => this.includes(project, file)));
    }
  }

  /**
   * Keeps dependency context while filtering findings from known sources to the project's target scope.
   *
   * @param project - Project whose host and globs identify owned source documents.
   * @param result - Captured analysis with original document URIs.
   * @returns A copy with filtered reports. Reports for unknown source documents remain available.
   * @throws If a file URI in a report cannot be converted to a path.
   */
  private scopedResult(project: WorkspaceProject, result: AnalysisResult): AnalysisResult {
    return {
      ...result,
      reports: result.reports.filter((report) => {
        const uri = report.primary.location.uri;
        if (uri.startsWith("file:")) {
          const file = fileURLToPath(uri);
          if (project.analysis.host.supports(file)) {
            return this.includes(project, file);
          }
        }
        const source = result.graph?.sources.find((source) => source.document.uri === uri);
        if (!source) {
          return true;
        }
        const file = uri.startsWith("file:") ? fileURLToPath(uri) : source.resolverId;
        return this.includes(project, file);
      }),
    };
  }

  /**
   * Closes every project and waits for all cleanup attempts before reporting failures.
   *
   * @returns A promise that resolves after project cleanup.
   * @throws An AggregateError containing all project cleanup failures.
   */
  async close(): Promise<void> {
    const results = await Promise.allSettled(
      this.projects.map((project) => project.analysis.close()),
    );
    const errors = results.flatMap((result) =>
      result.status === "rejected" ? [result.reason] : [],
    );
    if (errors.length) {
      throw new AggregateError(errors, "Failed to close Stillwind analysis hosts");
    }
  }
}
