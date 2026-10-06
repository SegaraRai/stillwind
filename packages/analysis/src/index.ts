import type { UnverifiedRange } from "@stillwind/host";
import type { BindingReport } from "@stillwind/native";

/** Portable reports for one target at a captured project revision. */
export interface AnalysisResult {
  entry: string;
  revision: number;
  status: "complete" | "incomplete" | "stale" | "cancelled";
  reports: BindingReport[];
  dependencies?: AnalysisDependency[];
  unverified?: UnverifiedRange[];
  generation?: "complete" | "blocked";
  error?: string;
}

/** A dependency observed by the analysis engine's module resolver. */
export interface AnalysisDependency {
  importer: string;
  specifier: string;
  id?: string;
  status: "resolved" | "external" | "unresolved" | "unsupported" | "types";
}

/** Keeps the project's membership when several projects analyze the same target. */
export interface WorkspaceResult {
  project: number;
  config: string;
  result: AnalysisResult;
}

/** URI-based operations shared by Node hosts and browser compiler workers. */
export interface AnalysisWorkspaceService {
  update(uri: string, source: string, version: number): void | Promise<void>;
  closeDocument(uri: string): void | Promise<void>;
  analyze(uri: string, signal?: AbortSignal): Promise<WorkspaceResult[]>;
  close(): Promise<void>;
}

/** Initialization context without assumptions about filesystem paths or process state. */
export interface AnalysisWorkspaceContext {
  workspaceFolders: readonly string[];
}

/** Opened services and initialization errors, retained independently for each workspace. */
export interface OpenAnalysisWorkspaces {
  workspaces: ReadonlyMap<string, AnalysisWorkspaceService>;
  errors?: ReadonlyMap<string, string>;
}

/** Creates fresh services after the server has closed its previous services on reconfiguration. */
export type OpenWorkspaces = (context: AnalysisWorkspaceContext) => Promise<OpenAnalysisWorkspaces>;
