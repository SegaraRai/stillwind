/**
 * URI-based analysis contracts used by language servers and compiler workers.
 * Node adapters are available from `@stillwind/analysis/node`. `BrowserAnalysisWorkspace` in
 * the browser subpath compiles captured virtual files. Embeddings with their own compiler
 * services can use `createMemoryAnalysisWorkspace` in the memory subpath.
 */

import type { UnverifiedRange, EditorRequest, EditorResult } from "@stillwind/host";
import type { BindingReport } from "@stillwind/native";

export type { EditorRequest, EditorResult } from "@stillwind/host";

/** Analysis results tied to one target and the project revision captured for that request. */
export interface AnalysisResult {
  /** Entry identity supplied to the service. Node hosts use a file path. Memory hosts use a URI. */
  entry: string;
  /** Project change counter captured before analysis. Document changes make older results stale. */
  revision: number;
  /**
   * Complete means analysis had the required inputs, even when reports contain compiler errors.
   * Incomplete means some inputs or host operations failed. Stale and cancelled results omit reports.
   */
  status: "complete" | "incomplete" | "stale" | "cancelled";
  /** Findings for this project's scope. Locations use original documents and zero-based UTF-16 positions. */
  reports: BindingReport[];
  /** Imports observed by semantic traversal. Omission means no dependency information was returned. */
  dependencies?: AnalysisDependency[];
  /** Original UTF-8 ranges whose semantic information was unavailable. */
  unverified?: UnverifiedRange[];
  /** Whether code generation has the required validated facts and no compiler errors. */
  generation?: "complete" | "blocked";
  /** Caught host or compiler exception as text. Compiler error reports alone do not populate it. */
  error?: string;
}

/** A dependency observed by the analysis engine's module resolver. */
export interface AnalysisDependency {
  /** Full resolver identity of the importing source. */
  importer: string;
  /** Authored import request. */
  specifier: string;
  /** Resolved dependency identity, omitted when the request could not be resolved. */
  id?: string;
  /**
   * Resolved sources are traversed. External modules and dependencies handled through checker
   * declarations (`types`) are skipped.
   * Unresolved requests and unsupported sources leave analysis incomplete.
   */
  status: "resolved" | "external" | "unresolved" | "unsupported" | "types";
}

/** Keeps the project's membership when several projects analyze the same target. */
export interface WorkspaceResult {
  /** Zero-based project index within this configuration. Memory services use zero. */
  project: number;
  /** Configuration identity for grouping findings. It can be an absolute file path or an embedding's label. */
  config: string;
  /** Captured analysis for this project, retained separately from overlapping projects. */
  result: AnalysisResult;
}

/** Collects distinct reports per project while preserving their analysis order. */
export function lintReports(results: readonly WorkspaceResult[]) {
  const seen = new Set<string>();
  return results.flatMap((entry) =>
    entry.result.reports.flatMap((report) => {
      const key = JSON.stringify([entry.project, report]);
      if (seen.has(key)) {
        return [];
      }
      seen.add(key);
      return [{ project: entry.project, report }];
    }),
  );
}

/** URI-based operations shared by Node hosts and browser compiler workers. */
export interface AnalysisWorkspaceService {
  /**
   * Queries optional editor features against captured original documents.
   * @param uri - Target document URI within a configured project.
   * @param request - Operation using original UTF-16 offsets.
   * @param signal - Cancellation discards obsolete results.
   * @returns Independent project answers. Absence of this method means diagnostic-only support.
   */
  query?(
    uri: string,
    request: EditorRequest,
    signal?: AbortSignal,
  ): Promise<WorkspaceEditorResult[]>;
  /**
   * Replaces an open document's buffer in every project that may depend on it.
   *
   * @param uri - Document URI, including documents outside the project's target scope.
   * @param source - Complete original document text.
   * @param version - Safe integer that increases while the document is open. Repeating identical
   * text at the same version can be accepted as an unchanged update.
   * @returns Nothing, or a promise that resolves when subsequent analysis can use the buffer.
   * @throws If the version is invalid or does not increase, or the service cannot store the buffer.
   */
  update(uri: string, source: string, version: number): void | Promise<void>;
  /**
   * Removes an open buffer so future reads can use the underlying source provider.
   *
   * @param uri - Document URI. An unknown document has no effect.
   * @returns Nothing, or a promise that resolves when the buffer has been removed.
   */
  closeDocument(uri: string): void | Promise<void>;
  /**
   * Analyzes a target against captured buffers and keeps each matching project's result.
   *
   * @param uri - Target document URI.
   * @param signal - Optional cancellation signal. Services may discard results without interrupting
   * compiler work that already started.
   * @returns Results for matching projects, or an empty list when the URI is outside their scopes
   * or uses a scheme that the service does not handle.
   * @throws If the service is closed or its transport fails. Source failures may instead be returned
   * as incomplete results.
   */
  analyze(uri: string, signal?: AbortSignal): Promise<WorkspaceResult[]>;
  /**
   * Releases analysis resources after pending work. Call before replacing a workspace service.
   *
   * @returns A promise that resolves after cleanup completes.
   * @throws If cleanup fails.
   */
  close(): Promise<void>;
}

/** Keeps the project identity and source fingerprint of an editor answer. */
export interface WorkspaceEditorResult {
  /** Zero-based project index in the evaluated configuration. */
  project: number;
  /** Configuration identity used to distinguish overlapping project answers. */
  config: string;
  /** Data computed from an immutable source and design-system snapshot. */
  result: EditorResult;
}

/** Initialization context without assumptions about filesystem paths or process state. */
export interface AnalysisWorkspaceContext {
  /** Workspace directory URIs supplied by LSP initialization or the embedding application. */
  workspaceFolders: readonly string[];
  /** Embedding-specific files or settings supplied with the LSP initialize request. */
  initializationOptions?: unknown;
}

/** Opened services and initialization errors, retained independently for each workspace. */
export interface OpenAnalysisWorkspaces {
  /** Owned services keyed by configuration identity. The consumer closes them before reopening. */
  workspaces: ReadonlyMap<string, AnalysisWorkspaceService>;
  /** Initialization failure messages keyed by workspace or configuration identity. */
  errors?: ReadonlyMap<string, string>;
}

/**
 * Creates fresh services after the language server closes its previous services.
 *
 * @param context - Current workspace directory URIs, possibly empty.
 * @returns Independently opened services and any initialization errors. Successful services remain
 * usable when another workspace cannot be opened.
 */
export type OpenWorkspaces = (context: AnalysisWorkspaceContext) => Promise<OpenAnalysisWorkspaces>;
