import path from "node:path";
import { fileURLToPath } from "node:url";

import { captureProjectConfig, type CapturedProjectConfig } from "@stillwind/config";
import { loadConfiguration } from "@stillwind/config/node";
import { minimatch } from "minimatch";
import { glob } from "tinyglobby";

import type { EditorRequest, EditorResult } from "../editor/index.ts";
import { AnalysisProject, isStylesheet, sameFile, type AnalysisResult } from "./analysis.ts";
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
  /** Absolute path evaluated when this workspace opened. Reopen the workspace to apply configuration changes. */
  readonly configFile: string;
  /** Fixed project records in configuration order. Each analysis service owns mutable state. */
  readonly projects: readonly WorkspaceProject[];

  /**
   * Retains the evaluated configuration and takes ownership of its initialized projects.
   * @param configFile - Absolute configuration path.
   * @param projects - Successfully opened hosts in configuration order.
   */
  private constructor(configFile: string, projects: WorkspaceProject[]) {
    this.configFile = configFile;
    this.projects = projects;
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
    const { config } = await loadConfiguration(configFile);
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
      return new AnalysisWorkspace(configFile, projects);
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
   * @returns Whether the file is the CSS entry or an imported file that the CSS host marks for
   * lint. When the CSS graph cannot load, no stylesheet is linted, and script analysis reports
   * the failure.
   */
  async lintsStylesheet(project: WorkspaceProject, file: string): Promise<boolean> {
    if (!isStylesheet(file)) {
      return false;
    }
    const sources = await project.analysis.stylesheets().catch(() => []);
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
   * @param file - Absolute target path used for independent scope checks.
   * @param request - Operation in the original document's UTF-16 coordinates.
   * @param signal - Optional cancellation forwarded to each project's queued query.
   * @returns Available answers in configuration order, including overlapping project memberships.
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
      if (!this.includes(project, file) && !(await this.lintsStylesheet(project, file))) {
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
  }

  /**
   * Removes a buffer from every project so later reads use its source provider.
   *
   * @param file - File path resolved in the same way as update. Unknown buffers have no effect.
   */
  closeDocument(file: string): void {
    for (const project of this.projects) {
      project.analysis.closeDocument(file);
    }
  }

  /** Advances every project revision and requests CSS and resolver cache refresh before its next analysis. */
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
      for (const file of await this.targets(project, patterns, { stylesheets: true })) {
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
   * Keeps dependency context while filtering findings from known sources to the project's target scope.
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
