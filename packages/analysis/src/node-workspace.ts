import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

import { findConfiguration, findDocumentConfiguration } from "@stillwind/config/node";

import { AnalysisWorker } from "./worker-client.ts";
import type {
  AnalysisWorkspaceContext,
  AnalysisWorkspaceService,
  OpenAnalysisWorkspaces,
  WorkspaceEditorResult,
} from "./workspace.ts";

/**
 * Lists the directories that bound configuration discovery.
 *
 * @param context - Workspace directory URIs.
 * @returns Absolute paths of the file URIs, or the process working directory for an empty list.
 * @throws If converting a malformed file URI fails.
 */
function folderPaths(context: AnalysisWorkspaceContext): string[] {
  return context.workspaceFolders.length
    ? context.workspaceFolders
        .filter((uri) => uri.startsWith("file:"))
        .map((uri) => fileURLToPath(uri))
    : [process.cwd()];
}

/**
 * Opens one isolated Node analysis process for each distinct configuration.
 *
 * @param context - Workspace directory URIs. File URIs use upward configuration discovery, and a
 * folder without a configuration opens no service. Other URI schemes cannot open Node workspaces.
 * An empty list uses the process working directory.
 * @param config - Optional explicit configuration path, resolved from the process working directory.
 * When supplied, it replaces discovery from workspace folders.
 * @returns Services keyed by absolute configuration path and separate initialization errors.
 * Services accept file URIs, forward buffers to their workers, and discard cancelled results.
 * @throws If converting a malformed file URI or closing a failed worker throws.
 */
export async function openNodeWorkspaces(
  context: AnalysisWorkspaceContext,
  config?: string,
): Promise<OpenAnalysisWorkspaces> {
  const workspaces = new Map<string, AnalysisWorkspaceService>();
  const errors = new Map<string, string>();

  const paths = config
    ? [path.resolve(config)]
    : folderPaths(context).map((folder) => findConfiguration(folder));
  for (const file of new Set(paths.filter((file): file is string => Boolean(file)))) {
    await openConfiguration(file, workspaces, errors);
  }

  return {
    workspaces,
    errors,
  };
}

/**
 * Opens the nearest configuration of a document that no open service covers.
 *
 * @param uri - Document URI. Other schemes than `file:` open nothing.
 * @param context - Workspace directory URIs that bound the search.
 * @param known - Configuration paths that already have a service or an attempt.
 * @returns The new service and any initialization error, or undefined when the document has no
 * new configuration.
 * @throws If converting a malformed file URI or closing a failed worker throws.
 */
export async function discoverNodeWorkspace(
  uri: string,
  context: AnalysisWorkspaceContext,
  known: ReadonlySet<string>,
): Promise<OpenAnalysisWorkspaces | undefined> {
  if (!uri.startsWith("file:")) {
    return undefined;
  }
  const file = findDocumentConfiguration(fileURLToPath(uri), folderPaths(context));
  if (!file || known.has(file)) {
    return undefined;
  }
  const workspaces = new Map<string, AnalysisWorkspaceService>();
  const errors = new Map<string, string>();
  await openConfiguration(file, workspaces, errors);
  return {
    workspaces,
    errors,
  };
}

/**
 * Starts one analysis process and records its service or its initialization failure.
 *
 * @param file - Absolute configuration path.
 * @param workspaces - Receives the service keyed by file.
 * @param errors - Receives the failure message keyed by file.
 * @throws If closing a failed worker throws.
 */
async function openConfiguration(
  file: string,
  workspaces: Map<string, AnalysisWorkspaceService>,
  errors: Map<string, string>,
): Promise<void> {
  const worker = new AnalysisWorker();
  try {
    await worker.open(file);
    /**
     * Selects document identities that can be forwarded to the filesystem worker.
     *
     * @param uri - Document URI whose syntax is validated later by `fileURLToPath`.
     * @returns Whether the URI begins with the `file:` scheme.
     */
    const accepts = (uri: string) => uri.startsWith("file:");
    workspaces.set(file, {
      refresh: (uris) =>
        worker.request<boolean>({
          method: "refresh",
          files: uris.filter(accepts).map((uri) => fileURLToPath(uri)),
        }),
      affects: (uris) =>
        worker.request<boolean>({
          method: "affects",
          files: uris.filter(accepts).map((uri) => fileURLToPath(uri)),
        }),
      prepare: async (uris, signal) => {
        if (!signal?.aborted) {
          await worker.request({
            method: "prepare",
            files: uris.filter(accepts).map((uri) => fileURLToPath(uri)),
          });
        }
      },
      query: async (uri, request, signal) => {
        if (!accepts(uri) || signal?.aborted) {
          return [];
        }
        const results = await worker.request<WorkspaceEditorResult[]>({
          method: "query",
          file: fileURLToPath(uri),
          request,
        });
        return signal?.aborted ? [] : results;
      },
      update: async (uri, source, version) => {
        if (accepts(uri)) {
          await worker.request({
            method: "update",
            file: fileURLToPath(uri),
            source,
            version,
          });
        }
      },
      analyze: async (uri, signal) => {
        if (!accepts(uri) || signal?.aborted) {
          return [];
        }
        const results = await worker.analyze(fileURLToPath(uri));
        return signal?.aborted
          ? []
          : results.map(({ project, config, result }) => ({
              project,
              config,
              result: {
                entry: result.entry,
                revision: result.revision,
                status: result.status,
                reports: result.reports,
                error: result.error,
                dependencies: result.graph?.dependencies,
                unverified: result.graph?.unverified,
                generation: result.graph?.generation,
              },
            }));
      },
      closeDocument: async (uri) => {
        if (accepts(uri)) {
          await worker.request({
            method: "closeDocument",
            file: fileURLToPath(uri),
          });
        }
      },
      close: async () => {
        await worker.close();
      },
    });
  } catch (error) {
    errors.set(file, String(error));
    await worker.close();
  }
}
