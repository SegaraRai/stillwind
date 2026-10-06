import type {
  AnalysisWorkspaceService,
  OpenWorkspaces,
  WorkspaceResult,
} from "@stillwind/analysis";
import {
  CodeActionKind,
  DiagnosticSeverity,
  DidChangeWatchedFilesNotification,
  TextDocuments,
  TextDocumentSyncKind,
  type CodeAction,
  type Connection,
  type Diagnostic,
  type InitializeParams,
  type TextDocumentEdit,
} from "vscode-languageserver";
import { TextDocument } from "vscode-languageserver-textdocument";

/** Supplies analysis services and delegates transport exit to the embedding application. */
export interface LanguageServerOptions {
  openWorkspaces: OpenWorkspaces;
  onExit?(shutdown: boolean): void;
}

/** Registers LSP handlers on a caller-owned connection. The caller starts and disposes its transport. */
export function createLanguageServer(
  connection: Connection,
  options: LanguageServerOptions,
): {
  dispose(): Promise<void>;
} {
  const documents = new TextDocuments(TextDocument);
  const workspaces = new Map<string, AnalysisWorkspaceService>();
  const errors = new Map<string, string>();
  const cache = new Map<string, WorkspaceResult[]>();
  const published = new Set<string>();

  let roots: string[] = [];
  let generation = 0;
  let controls = Promise.resolve();
  let timer: ReturnType<typeof setTimeout> | undefined;
  let shutdown = false;
  let foldersSupported = false;
  let dynamicWatch = false;
  let active: AbortController | undefined;
  let disposal: Promise<void> | undefined;

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

  const open = async () => {
    await Promise.all([...workspaces.values()].map((workspace) => workspace.close()));
    workspaces.clear();
    errors.clear();
    const opened = await options.openWorkspaces({
      workspaceFolders: roots,
    });
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

  const publish = () => {
    const grouped = new Map<string, Diagnostic[]>();
    const seen = new Set<string>();
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
              location: { uri: label.location.uri, range: range(label.location) },
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
    for (const document of documents.all()) {
      for (const [file, error] of errors) {
        add(document.uri, {
          range: range(),
          severity: DiagnosticSeverity.Error,
          source: "stillwind",
          code: "SW_HOST",
          message: `${file}: ${error}`,
        });
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

  const analyze = async (revision: number) => {
    await controls;
    if (shutdown || revision !== generation) {
      return;
    }
    const cancellation = new AbortController();
    active = cancellation;
    const next = new Map<string, WorkspaceResult[]>();
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
    }
  };

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

  connection.onInitialize(async (params: InitializeParams) => {
    foldersSupported = params.capabilities.workspace?.workspaceFolders === true;
    dynamicWatch =
      params.capabilities.workspace?.didChangeWatchedFiles?.dynamicRegistration === true;
    roots =
      params.workspaceFolders?.map((folder) => folder.uri) ??
      (params.rootUri ? [params.rootUri] : []);
    await open();
    return {
      capabilities: {
        textDocumentSync: TextDocumentSyncKind.Incremental,
        codeActionProvider: {
          codeActionKinds: [CodeActionKind.QuickFix],
        },
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
  documents.onDidSave(() => {
    control(open);
    schedule();
  });
  connection.onDidChangeWatchedFiles(() => {
    control(open);
    schedule();
  });
  connection.onDidChangeConfiguration(() => {
    control(open);
    schedule();
  });
  connection.onCodeAction(async (params, token) => {
    const result: CodeAction[] = [];
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
    dispose,
  };
}

function range(location?: {
  startLine: number;
  startColumn: number;
  endLine: number;
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
