import type {
  AnalysisWorkspaceService,
  DiscoverWorkspace,
  OpenAnalysisWorkspaces,
  OpenWorkspaces,
  WorkspaceEditorResult,
  WorkspaceResult,
} from "@stillwind/analysis";
import {
  CodeActionKind,
  DiagnosticSeverity,
  DidChangeWatchedFilesNotification,
  MessageType,
  ShowMessageNotification,
  TextDocuments,
  TextDocumentSyncKind,
  type CancellationToken,
  type CodeAction,
  type Connection,
  type Diagnostic,
  type InitializeParams,
  type TextDocumentEdit,
} from "vscode-languageserver";
import { TextDocument } from "vscode-languageserver-textdocument";

import {
  editorCodeActionKinds,
  registerEditorFeatures,
  sourceActions,
  type EditorQuery,
} from "./editor.ts";

/** Supplies analysis services and delegates transport exit to the embedding application. */
export interface LanguageServerOptions {
  /**
   * Opens services at initialization and after saves, watched file changes, configuration changes,
   * or workspace folder changes. Previous services are closed first. Open buffers are replayed.
   */
  openWorkspaces: OpenWorkspaces;
  /**
   * Opens a configuration for an open document that no service analyzes. The server calls this once
   * for each document until services reopen. Supplying it advertises editor features before any
   * service exists. Omission leaves such documents without analysis.
   */
  discoverWorkspace?: DiscoverWorkspace;
  /**
   * Converts a service identity to the URI of its configuration file, where the server publishes
   * that service's errors.
   *
   * @param id - Key from a workspace map or an error map.
   * @param folders - Current workspace directory URIs.
   * @returns The file URI, or undefined when the identity names no file. The server then only logs
   * the error and notifies the user. Omitting this option has the same effect.
   */
  configurationUri?(id: string, folders: readonly string[]): string | undefined;
  /**
   * Handles the LSP exit notification using the embedding's transport or process policy.
   *
   * @param shutdown - Whether shutdown or explicit disposal was requested before exit.
   */
  onExit?(shutdown: boolean): void;
}

/**
 * Registers synchronization, diagnostics, fixes, hover, completion, colors, and sorting.
 * Analysis uses captured buffers and suppresses obsolete results. Quick fixes require every edited
 * document to be open and to match the report's source fingerprint.
 *
 * @param connection - Caller-owned connection. Call listen after registration and dispose the
 * transport when finished. Diagnostic locations must use original documents and UTF-16 positions.
 * @param options - Workspace factory and optional exit handler. This server owns the opened services.
 * @returns Cleanup for the analysis services. It does not dispose the connection or its transport.
 */
export function createLanguageServer(
  connection: Connection,
  options: LanguageServerOptions,
): {
  /**
   * Waits for document and workspace changes already queued by the server.
   *
   * @returns The current control queue's completion promise. Queued failures are logged.
   */
  ready(): Promise<void>;
  /**
   * Reopens workspace configuration and files, replays current buffers, and schedules analysis.
   *
   * @returns Completion of the queued reload. Reload failures are logged, and analysis runs separately.
   */
  reload(): Promise<void>;
  /**
   * Stops scheduling analysis, cancels publication, and closes services after queued control work.
   *
   * @returns The same cleanup promise on repeated calls.
   * @throws An AggregateError if any service fails to close.
   */
  dispose(): Promise<void>;
} {
  const documents = new TextDocuments(TextDocument);
  const workspaces = new Map<string, AnalysisWorkspaceService>();
  const errors = new Map<string, string>();
  const cache = new Map<string, WorkspaceResult[]>();
  const published = new Set<string>();
  /** Documents already passed to discovery, which keeps later analysis passes from searching again. */
  const discovered = new Set<string>();
  /** Current error messages that the user has seen, which keeps each from notifying again. */
  const reported = new Set<string>();

  let roots: string[] = [];
  let initializationOptions: unknown;
  let registryDirectives = false;
  let snippets = false;
  let generation = 0;
  let controls = Promise.resolve();
  let timer: ReturnType<typeof setTimeout> | undefined;
  let shutdown = false;
  let foldersSupported = false;
  let dynamicWatch = false;
  let active: AbortController | undefined;
  let disposal: Promise<void> | undefined;

  /**
   * Invalidates current results and serializes a workspace or document mutation.
   * Callback failures are logged and absorbed so subsequent mutations can run.
   *
   * @param action - Mutation to execute after previous control work. Shutdown skips registration.
   */
  const control = (action: () => Promise<void>) => {
    if (shutdown) {
      return;
    }
    active?.abort();
    generation++;
    cache.clear();
    controls = controls.then(action).catch((error: unknown) => {
      connection.console.error(String(error));
    });
  };

  /**
   * Takes ownership of opened services and replays every open buffer into them.
   *
   * @param opened - Services and initialization errors from the workspace factory or discovery.
   * @returns Completion of buffer replay.
   * @throws If buffer replay fails.
   */
  const adopt = async (opened: OpenAnalysisWorkspaces) => {
    for (const [id, error] of opened.errors ?? []) {
      errors.set(id, error);
    }
    for (const [id, workspace] of opened.workspaces) {
      workspaces.set(id, workspace);
      for (const document of documents.all()) {
        await workspace.update(document.uri, document.getText(), document.version);
      }
    }
  };

  /**
   * Closes existing services, opens the current roots, and replays every open buffer.
   *
   * @returns Completion of service replacement and buffer replay.
   * @throws If service closure, the workspace factory, or buffer replay fails.
   */
  const open = async () => {
    await Promise.all([...workspaces.values()].map((workspace) => workspace.close()));
    workspaces.clear();
    errors.clear();
    discovered.clear();
    await adopt(
      await options.openWorkspaces({
        workspaceFolders: roots,
        initializationOptions,
      }),
    );
  };

  /**
   * Opens configurations for documents that the last analysis pass left without results.
   *
   * @param uris - Document URIs. A document closed in the meantime is skipped.
   * @returns Completion of discovery and buffer replay.
   * @throws If discovery or buffer replay fails.
   */
  const discover = async (uris: readonly string[]) => {
    for (const uri of uris) {
      if (!documents.get(uri)) {
        continue;
      }
      const opened = await options.discoverWorkspace?.(
        uri,
        {
          workspaceFolders: roots,
          initializationOptions,
        },
        new Set([...workspaces.keys(), ...errors.keys()]),
      );
      if (opened) {
        await adopt(opened);
      }
    }
  };

  /**
   * Sends deduplicated cached diagnostics and workspace errors, then records their document URIs.
   * Empty notifications clear diagnostics for previously published or currently open documents.
   */
  const publish = () => {
    const grouped = new Map<string, Diagnostic[]>();
    const seen = new Set<string>();
    /**
     * Appends a diagnostic only once for its document and complete serialized value.
     *
     * @param uri - Original document URI used to group publication.
     * @param diagnostic - Diagnostic with original LSP positions.
     */
    const add = (uri: string, diagnostic: Diagnostic) => {
      const key = JSON.stringify([uri, diagnostic]);
      if (!seen.has(key)) {
        seen.add(key);
        const list = grouped.get(uri) ?? [];
        list.push(diagnostic);
        grouped.set(uri, list);
      }
    };
    for (const [uri, results] of cache) {
      for (const entry of results) {
        const source = `stillwind:${entry.config}:${entry.project + 1}`;
        for (const report of entry.result.reports) {
          const location = report.primary.location;
          add(location.uri, {
            range: range(location),
            severity:
              report.severity === "error" ? DiagnosticSeverity.Error : DiagnosticSeverity.Warning,
            source,
            code: report.code,
            message: [report.message, ...report.notes].join("\n"),
            relatedInformation: report.related.map((label) => ({
              location: {
                uri: label.location.uri,
                range: range(label.location),
              },
              message: label.message ?? report.message,
            })),
          });
        }
        if (entry.result.status === "incomplete") {
          const missing =
            entry.result.dependencies
              ?.filter(
                (dependency) =>
                  dependency.status === "unresolved" || dependency.status === "unsupported",
              )
              .map(
                (dependency) =>
                  `${dependency.status}: ${dependency.specifier} from ${dependency.importer}`,
              ) ?? [];
          add(uri, {
            range: range(),
            severity: entry.result.error
              ? DiagnosticSeverity.Error
              : DiagnosticSeverity.Information,
            source,
            code: entry.result.error ? "SW_HOST" : "SW_UNVERIFIED",
            message: [
              entry.result.error ?? "Stillwind analysis is incomplete",
              ...missing,
              ...(entry.result.unverified ?? []).map((item) => `${item.moduleId}: ${item.reason}`),
            ].join("\n"),
          });
        }
      }
    }
    const current = new Set<string>();
    for (const [file, error] of errors) {
      const message = `${file}: ${error}`;
      const uri = options.configurationUri?.(file, roots);
      if (uri) {
        add(uri, {
          range: range(),
          severity: DiagnosticSeverity.Error,
          source: "stillwind",
          code: "SW_HOST",
          message: error,
        });
      }
      current.add(message);
      if (!reported.has(message)) {
        reported.add(message);
        connection.console.error(message);
        void connection.sendNotification(ShowMessageNotification.type, {
          type: MessageType.Warning,
          message: `Stillwind: ${message}`,
        });
      }
    }
    for (const message of reported) {
      if (!current.has(message)) {
        reported.delete(message);
      }
    }
    for (const uri of new Set([
      ...published,
      ...grouped.keys(),
      ...documents.all().map((document) => document.uri),
    ])) {
      void connection.sendDiagnostics({
        uri,
        version: documents.get(uri)?.version,
        diagnostics: grouped.get(uri) ?? [],
      });
    }
    published.clear();
    for (const uri of grouped.keys()) {
      published.add(uri);
    }
  };

  /**
   * Analyzes open buffers after queued mutations and publishes only a current, non-stale batch.
   * Service failures become workspace errors. An obsolete or cancelled batch leaves the cache unchanged.
   *
   * @param revision - Control generation captured when analysis was scheduled.
   * @returns Completion of the analysis pass or its early discard.
   */
  const analyze = async (revision: number) => {
    await controls;
    if (shutdown || revision !== generation) {
      return;
    }
    const cancellation = new AbortController();
    active = cancellation;
    const next = new Map<string, WorkspaceResult[]>();
    for (const [file, workspace] of workspaces) {
      try {
        await workspace.prepare?.(
          documents.all().map((document) => document.uri),
          cancellation.signal,
        );
      } catch (error) {
        errors.set(file, String(error));
      }
      if (revision !== generation || shutdown) {
        return;
      }
    }
    for (const document of documents.all()) {
      const results: WorkspaceResult[] = [];
      for (const [file, workspace] of workspaces) {
        try {
          results.push(...(await workspace.analyze(document.uri, cancellation.signal)));
          errors.delete(file);
        } catch (error) {
          errors.set(file, String(error));
        }
      }
      if (revision !== generation || shutdown) {
        return;
      }
      if (
        results.some(({ result }) => result.status === "stale" || result.status === "cancelled")
      ) {
        return;
      }
      next.set(document.uri, results);
    }
    if (revision === generation) {
      cache.clear();
      for (const [uri, result] of next) {
        cache.set(uri, result);
      }
      publish();
      const uncovered = options.discoverWorkspace
        ? [...next].flatMap(([uri, results]) =>
            results.length || discovered.has(uri) ? [] : [uri],
          )
        : [];
      if (uncovered.length) {
        for (const uri of uncovered) {
          discovered.add(uri);
        }
        control(() => discover(uncovered));
        schedule();
      }
    }
  };

  /** Replaces the pending 40 ms timer with an analysis pass for the current control generation. */
  const schedule = () => {
    if (timer) {
      clearTimeout(timer);
    }
    const revision = generation;
    timer = setTimeout(() => {
      void analyze(revision).catch((error: unknown) => {
        connection.console.error(String(error));
      });
    }, 40);
  };

  /**
   * Queries services after queued mutations and checks document versions and source fingerprints.
   * Query failures are logged, and the cancellation subscription is released after the request.
   *
   * @param uri - URI of the open original document.
   * @param request - Editor operation expressed in that document's UTF-16 coordinates.
   * @param token - LSP cancellation forwarded through an AbortSignal.
   * @returns Current project results, or an empty list after changes, cancellation, or failure.
   */
  const query: EditorQuery = async (uri, request, token: CancellationToken) => {
    await controls;
    if (request.kind === "completion" || request.kind === "resolve") {
      request = {
        ...request,
        registryDirectives,
        snippets,
      };
    }
    const document = documents.get(uri);
    if (!document || shutdown || token.isCancellationRequested) {
      return [];
    }
    const version = document.version;
    const revision = generation;
    const controller = new AbortController();
    const subscription = token.onCancellationRequested(() => controller.abort());
    const results: WorkspaceEditorResult[] = [];
    try {
      for (const workspace of workspaces.values()) {
        if (workspace.query) {
          results.push(...(await workspace.query(uri, request, controller.signal)));
        }
      }
      if (
        token.isCancellationRequested ||
        revision !== generation ||
        documents.get(uri)?.version !== version
      ) {
        return [];
      }
      const { fingerprint } = await import("@stillwind/native");
      return results.filter((entry) => entry.result.revision === fingerprint(document.getText()));
    } catch (error) {
      connection.console.error(String(error));
      return [];
    } finally {
      subscription.dispose();
    }
  };
  registerEditorFeatures(connection, documents, query);

  connection.onInitialize(async (params: InitializeParams) => {
    initializationOptions = params.initializationOptions;
    registryDirectives = completionRegistrySetting(initializationOptions) ?? false;
    snippets =
      params.capabilities.textDocument?.completion?.completionItem?.snippetSupport === true;
    foldersSupported = params.capabilities.workspace?.workspaceFolders === true;
    dynamicWatch =
      params.capabilities.workspace?.didChangeWatchedFiles?.dynamicRegistration === true;
    roots =
      params.workspaceFolders?.map((folder) => folder.uri) ??
      (params.rootUri ? [params.rootUri] : []);
    await open();
    const editor =
      options.discoverWorkspace != null ||
      [...workspaces.values()].some((workspace) => workspace.query != null);
    return {
      capabilities: {
        textDocumentSync: TextDocumentSyncKind.Incremental,
        codeActionProvider: {
          codeActionKinds: [CodeActionKind.QuickFix, ...(editor ? editorCodeActionKinds : [])],
        },
        hoverProvider: editor,
        definitionProvider: editor,
        completionProvider: editor
          ? {
              resolveProvider: true,
              triggerCharacters: ["@", ":", "-", "/", "[", "(", " "],
            }
          : undefined,
        colorProvider: editor,
        documentLinkProvider: editor
          ? {
              resolveProvider: false,
            }
          : undefined,
        workspace: {
          workspaceFolders: {
            supported: true,
            changeNotifications: true,
          },
        },
      },
      serverInfo: {
        name: "Stillwind",
      },
    };
  });
  connection.onInitialized(async () => {
    if (foldersSupported) {
      connection.workspace.onDidChangeWorkspaceFolders(({ added, removed }) => {
        const deleted = new Set(removed.map((folder) => folder.uri));
        roots = [
          ...roots.filter((root) => !deleted.has(root)),
          ...added.map((folder) => folder.uri),
        ];
        control(open);
        schedule();
      });
    }
    if (dynamicWatch) {
      await connection.client.register(DidChangeWatchedFilesNotification.type, {
        watchers: [
          {
            globPattern: "**/*",
          },
        ],
      });
    }
    schedule();
  });
  documents.onDidChangeContent(({ document }) => {
    control(async () => {
      if (documents.get(document.uri)?.version !== document.version) {
        return;
      }
      for (const workspace of workspaces.values()) {
        await workspace.update(document.uri, document.getText(), document.version);
      }
    });
    schedule();
  });
  documents.onDidClose(({ document }) => {
    control(async () => {
      for (const workspace of workspaces.values()) {
        await workspace.closeDocument(document.uri);
      }
    });
    schedule();
  });
  /**
   * Reopens the workspaces if any service cannot reuse its current buffers after a save or file
   * change. Without services, reopening lets a new or corrected configuration take effect.
   *
   * @param uris - Document URIs from the save or watch notification.
   * @returns Completion of the checks and any required workspace replacement.
   * @throws If a service check or workspace replacement fails.
   */
  const refresh = async (uris: readonly string[]) => {
    if (!workspaces.size) {
      await open();
      return;
    }
    for (const workspace of workspaces.values()) {
      if (!(await workspace.refresh?.(uris))) {
        await open();
        break;
      }
    }
  };
  documents.onDidSave(({ document }) => {
    control(() => refresh([document.uri]));
    schedule();
  });
  connection.onDidChangeWatchedFiles(({ changes }) => {
    const uris = changes.map((change) => change.uri).filter((uri) => !isGeneratedPath(uri, roots));
    if (uris.length) {
      control(() => refresh(uris));
      schedule();
    }
  });
  connection.onDidChangeConfiguration(({ settings }) => {
    control(async () => {
      registryDirectives =
        completionRegistrySetting(settings?.stillwind ?? settings) ?? registryDirectives;
      await open();
    });
    schedule();
  });
  connection.onCodeAction(async (params, token) => {
    const document = documents.get(params.textDocument.uri);
    const result: CodeAction[] = document
      ? await sourceActions(document, params.context.only, token, query)
      : [];
    if (
      params.context.only &&
      !params.context.only.some(
        (kind) =>
          CodeActionKind.QuickFix === kind || CodeActionKind.QuickFix.startsWith(`${kind}.`),
      )
    ) {
      return result;
    }
    if (token.isCancellationRequested) {
      return result;
    }
    const { fingerprint } = await import("@stillwind/native");
    if (token.isCancellationRequested) {
      return result;
    }
    for (const entries of cache.values()) {
      for (const entry of entries) {
        for (const report of entry.result.reports) {
          const primary = report.primary.location;
          if (
            primary.uri !== params.textDocument.uri ||
            primary.endLine < params.range.start.line ||
            primary.startLine > params.range.end.line
          ) {
            continue;
          }
          if (
            document &&
            (primary.endUtf16 < document.offsetAt(params.range.start) ||
              primary.startUtf16 > document.offsetAt(params.range.end))
          ) {
            continue;
          }
          for (const action of report.actions) {
            const changes = new Map<string, TextDocumentEdit>();
            let valid = true;
            for (const edit of action.edits) {
              const document = documents.get(edit.location.uri);
              if (!document || fingerprint(document.getText()) !== edit.location.revision) {
                valid = false;
                break;
              }
              const change = changes.get(document.uri) ?? {
                textDocument: {
                  uri: document.uri,
                  version: document.version,
                },
                edits: [],
              };
              change.edits.push({
                range: range(edit.location),
                newText: edit.replacement,
              });
              changes.set(document.uri, change);
            }
            if (valid && changes.size) {
              result.push({
                title: action.title,
                kind: CodeActionKind.QuickFix,
                edit: {
                  documentChanges: [...changes.values()],
                },
              });
            }
          }
        }
      }
    }
    return result.filter(
      (action, index) =>
        result.findIndex((candidate) => JSON.stringify(candidate) === JSON.stringify(action)) ===
        index,
    );
  });
  /**
   * Stops publication and closes all services after the control queue settles.
   *
   * @returns A shared cleanup promise, including on repeated calls.
   * @throws An AggregateError containing every service closure failure.
   */
  const dispose = (): Promise<void> => {
    disposal ??= (async () => {
      shutdown = true;
      active?.abort();
      generation++;
      cache.clear();
      if (timer) {
        clearTimeout(timer);
      }
      await controls;
      const closed = await Promise.allSettled(
        [...workspaces.values()].map((workspace) => workspace.close()),
      );
      workspaces.clear();
      const failed = closed.filter((result) => result.status === "rejected");
      if (failed.length) {
        throw new AggregateError(
          failed.map((result) => result.reason),
          "Failed to close language server analysis services",
        );
      }
    })();
    return disposal;
  };
  connection.onShutdown(dispose);
  connection.onExit(() => {
    options.onExit?.(shutdown);
  });
  documents.listen(connection);
  return {
    ready: () => controls,
    reload: async () => {
      control(open);
      schedule();
      await controls;
    },
    dispose,
  };
}

/**
 * Recognizes files that package managers and build tools write in bursts. Reacting to them would
 * restart analysis before it finishes.
 *
 * @param uri - URI from a watch notification.
 * @param roots - Workspace directory URIs. Only path segments beneath a root count, so a workspace
 * inside a dot directory stays visible.
 * @returns Whether a directory beneath a root is `node_modules`, `target`, or a dot directory.
 */
export function isGeneratedPath(uri: string, roots: readonly string[]): boolean {
  const root = roots.find((candidate) => uri.startsWith(`${candidate.replace(/\/$/, "")}/`));
  return (
    root !== undefined &&
    uri
      .slice(root.replace(/\/$/, "").length + 1)
      .split("/")
      .slice(0, -1)
      .some(
        (segment) =>
          segment === "node_modules" || segment === "target" || /^\.|^%2E/i.test(segment),
      )
  );
}

/**
 * Reads the editor-only completion field from initialization options or configuration settings.
 *
 * @param value - A protocol object that may contain `completion.registryDirectives`.
 * @returns The boolean setting, or undefined when the field is absent or invalid.
 */
function completionRegistrySetting(value: unknown): boolean | undefined {
  if (!value || typeof value !== "object") {
    return undefined;
  }
  const completion = (value as Record<string, unknown>).completion;
  if (!completion || typeof completion !== "object") {
    return undefined;
  }
  const registry = (completion as Record<string, unknown>).registryDirectives;
  return typeof registry === "boolean" ? registry : undefined;
}

/**
 * Copies a native diagnostic location into LSP positions.
 *
 * @param location - Original document positions with zero-based lines and UTF-16 columns.
 * @returns The half-open range, or an empty range at document start when no location is supplied.
 */
function range(location?: {
  /** Zero-based line containing the inclusive start. */
  startLine: number;
  /** UTF-16 column of the inclusive start. */
  startColumn: number;
  /** Zero-based line containing the exclusive end. */
  endLine: number;
  /** UTF-16 column of the exclusive end. */
  endColumn: number;
}) {
  return {
    start: {
      line: location?.startLine ?? 0,
      character: location?.startColumn ?? 0,
    },
    end: {
      line: location?.endLine ?? 0,
      character: location?.endColumn ?? 0,
    },
  };
}
