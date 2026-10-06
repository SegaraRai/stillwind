import path from "node:path";
import { fileURLToPath } from "node:url";

import type { AnalysisProjectConfig } from "@stillwind/config";
import { loadConfiguration } from "@stillwind/config/node";
import { minimatch } from "minimatch";
import { glob } from "tinyglobby";

import type { EditorRequest, EditorResult } from "../editor/index.ts";
import { AnalysisProject, type AnalysisResult } from "./analysis.ts";
import { createFileSystemAnalysisHost } from "./filesystem.ts";

/** A project's configuration and results, retained independently when scopes overlap. */
export interface WorkspaceProject {
  /** Evaluated project settings. Relative root and scope globs use configFile's directory and host root. */
  config: AnalysisProjectConfig;
  /** Absolute path of the configuration that owns this project. */
  configFile: string;
  /** Owned analysis state. Buffers outside this project's target scope can still be dependencies. */
  analysis: AnalysisProject;
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
  /** Owned project hosts in configuration order. The caller must not mutate this array. */
  readonly projects: WorkspaceProject[];

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
      for (const project of config.projects) {
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
    const matches = (pattern: string) => minimatch(name, pattern, { dot: true });
    return (
      project.config.files.some(matches) &&
      !["**/node_modules/**", "**/.git/**", ...(project.config.exclude ?? [])].some(matches)
    );
  }

  /**
   * Enumerates existing target files independently of dependency traversal.
   *
   * @param project - Project whose files and excludes globs are relative to its host root.
   * @param patterns - Optional additional include globs relative to the same root. An empty list
   * adds no filtering. These patterns cannot expand the configured scope.
   * @returns Sorted absolute target paths. Unsaved files absent from disk are not enumerated.
   * @throws If filesystem enumeration fails.
   */
  async targets(project: WorkspaceProject, patterns?: string[]): Promise<string[]> {
    const files = await glob(project.config.files, {
      cwd: project.analysis.host.root,
      absolute: true,
      onlyFiles: true,
      dot: true,
      ignore: ["**/node_modules/**", "**/.git/**", ...(project.config.exclude ?? [])],
    });
    return files
      .filter(
        (file) =>
          !patterns?.length ||
          patterns.some((pattern) =>
            minimatch(
              path.relative(project.analysis.host.root, file).replaceAll("\\", "/"),
              pattern,
              { dot: true },
            ),
          ),
      )
      .sort();
  }

  /** Keeps editor answers separate when several projects include the same target. */
  async query(
    file: string,
    request: EditorRequest,
    signal?: AbortSignal,
  ): Promise<
    {
      project: number;
      config: string;
      result: EditorResult;
    }[]
  > {
    const results: {
      project: number;
      config: string;
      result: EditorResult;
    }[] = [];
    for (const [index, project] of this.projects.entries()) {
      if (!this.includes(project, file)) {
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
   *
   * @param file - Absolute target path. The file need not exist on disk when an open buffer supplies it.
   * @param signal - Optional cancellation checked by each project's queued analysis.
   * @returns Results in project order, or an empty list when no project includes the target.
   * Dependency information remains available even when its reports lie outside the target scope.
   */
  async analyze(file: string, signal?: AbortSignal): Promise<WorkspaceResult[]> {
    const results: WorkspaceResult[] = [];
    for (const [index, project] of this.projects.entries()) {
      if (this.includes(project, file)) {
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
   * Analyzes every selected target, including files unreachable from an application entry.
   *
   * @param patterns - Optional additional globs relative to each project's root. Empty or omitted
   * filters check every configured target.
   * @returns Scoped results in project order, then sorted target order. Overlapping memberships remain separate.
   * @throws If target enumeration fails. Analysis failures are represented in individual results.
   */
  async lint(patterns?: string[]): Promise<WorkspaceResult[]> {
    const results: WorkspaceResult[] = [];
    for (const [index, project] of this.projects.entries()) {
      for (const file of await this.targets(project, patterns)) {
        results.push({
          project: index,
          config: project.configFile,
          result: this.scopedResult(project, await project.analysis.analyze(file)),
        });
      }
    }
    return results;
  }

  /** Keeps dependency context while publishing source diagnostics only for the configured scope. */
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
