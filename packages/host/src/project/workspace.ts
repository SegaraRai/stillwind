import { existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { createJiti } from "jiti";
import { minimatch } from "minimatch";
import { resolveConfig, type AnalysisProjectConfig } from "stillwind/config";
import { glob } from "tinyglobby";

import { AnalysisProject, type AnalysisResult } from "./analysis.ts";
import { createFileSystemAnalysisHost } from "./filesystem.ts";

/** A project's configuration and results, retained independently when scopes overlap. */
export interface WorkspaceProject {
  config: AnalysisProjectConfig;
  configFile: string;
  analysis: AnalysisProject;
}

/** Analysis results for one explicitly enumerated target in one project. */
export interface WorkspaceResult {
  project: number;
  config: string;
  result: AnalysisResult;
}

/** Loads configuration without a disk cache, then owns all selected project hosts. */
export class AnalysisWorkspace {
  readonly configFile: string;
  readonly projects: WorkspaceProject[];

  private constructor(configFile: string, projects: WorkspaceProject[]) {
    this.configFile = configFile;
    this.projects = projects;
  }

  /** Opens all projects, closing already opened hosts if any project fails to initialize. */
  static async open(configFile: string): Promise<AnalysisWorkspace> {
    configFile = path.resolve(configFile);
    const config = await resolveConfig(await loadConfiguration(configFile));
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

  /** Matches project scope independently of whether the file has been saved to disk. */
  includes(project: WorkspaceProject, file: string): boolean {
    const name = path.relative(project.analysis.host.root, file).replaceAll("\\", "/");
    const matches = (pattern: string) => minimatch(name, pattern, { dot: true });
    return (
      project.config.files.some(matches) &&
      !["**/node_modules/**", "**/.git/**", ...(project.config.exclude ?? [])].some(matches)
    );
  }

  /** Enumerates configured targets separately from imports demanded by the compiler. */
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

  /** Updates all hosts because an open file may be a dependency outside their target scope. */
  update(file: string, source: string, version: number): void {
    for (const project of this.projects) {
      project.analysis.update(file, source, version);
    }
  }

  closeDocument(file: string): void {
    for (const project of this.projects) {
      project.analysis.closeDocument(file);
    }
  }

  invalidate(): void {
    for (const project of this.projects) {
      project.analysis.invalidate();
    }
  }

  /** Returns one result for each project containing this target. */
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

  /** Analyzes every selected target, including files unreachable from an application entry. */
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

  /** Closes every host, reporting cleanup errors only after all hosts have been closed. */
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

/** Finds the nearest standalone configuration above a directory. */
export function findConfiguration(directory: string): string | undefined {
  let current = path.resolve(directory);
  for (;;) {
    const candidate = path.join(current, "stillwind.config.ts");
    if (existsSync(candidate)) {
      return candidate;
    }
    const parent = path.dirname(current);
    if (parent === current) {
      return undefined;
    }
    current = parent;
  }
}

/** Evaluates trusted host configuration with no persistent transpilation or module cache. */
async function loadConfiguration(file: string): Promise<unknown> {
  const jiti = createJiti(pathToFileURL(path.resolve(file)).href, {
    fsCache: false,
    moduleCache: false,
  });
  return jiti.import(file, {
    default: true,
  });
}
