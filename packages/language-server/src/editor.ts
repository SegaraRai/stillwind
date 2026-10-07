import type { EditorRequest, WorkspaceEditorResult } from "@stillwind/analysis";
import {
  CompletionItemKind,
  MarkupKind,
  type CancellationToken,
  type CodeAction,
  type Connection,
  type CompletionItem,
  type TextDocuments,
} from "vscode-languageserver";
import type { TextDocument } from "vscode-languageserver-textdocument";

const sortKind = "source.sort.stillwind";
const fixAllKind = "source.fixAll.stillwind";

export const editorCodeActionKinds = [fixAllKind, sortKind];

/** Queries the current original buffer; implementations discard changed revisions and cancellation. */
export type EditorQuery = (
  uri: string,
  request: EditorRequest,
  token: CancellationToken,
) => Promise<WorkspaceEditorResult[]>;

/** Registers standard editor requests. Transports and captured workspace ownership remain with the caller. */
export function registerEditorFeatures(
  connection: Connection,
  documents: TextDocuments<TextDocument>,
  query: EditorQuery,
): void {
  connection.onHover(async (params, token) => {
    const document = documents.get(params.textDocument.uri);
    if (!document) {
      return null;
    }
    const entries = await query(
      document.uri,
      {
        kind: "hover",
        offset: document.offsetAt(params.position),
      },
      token,
    );
    const answers = entries.filter((entry) => entry.result.hover);
    if (!answers.length) {
      return null;
    }
    const seen = new Set<string>();
    const contents = answers
      .flatMap((entry) => {
        const css = entry.result.hover!.css;
        if (seen.has(css)) {
          return [];
        }
        seen.add(css);
        return [
          `${answers.length > 1 ? `${entry.config} · project ${entry.project + 1}\n\n` : ""}\`\`\`css\n${css}\n\`\`\``,
        ];
      })
      .join("\n\n");
    return {
      contents: {
        kind: MarkupKind.Markdown,
        value: contents,
      },
      range: offsets(document, answers[0].result.hover!),
    };
  });
  connection.onCompletion(async (params, token) => {
    const document = documents.get(params.textDocument.uri);
    if (!document) {
      return [];
    }
    const version = document.version;
    const entries = await query(
      document.uri,
      {
        kind: "completion",
        offset: document.offsetAt(params.position),
      },
      token,
    );
    const items: CompletionItem[] = entries.flatMap((entry) =>
      (entry.result.completions ?? []).map((item) => ({
        label: item.label,
        kind: item.candidate.endsWith(":") ? CompletionItemKind.Module : CompletionItemKind.Value,
        textEdit: {
          range: offsets(document, item),
          newText: item.newText,
        },
        data: {
          uri: document.uri,
          version,
          revision: entry.result.revision,
          candidate: item.candidate,
          project: entry.project,
          config: entry.config,
        },
      })),
    );
    return items.filter(
      (item, index) =>
        items.findIndex(
          (other) =>
            other.label === item.label &&
            JSON.stringify(other.textEdit) === JSON.stringify(item.textEdit),
        ) === index,
    );
  });
  connection.onCompletionResolve(async (item, token) => {
    const data = item.data as
      | {
          uri?: unknown;
          version?: unknown;
          candidate?: unknown;
          config?: unknown;
          project?: unknown;
          revision?: unknown;
        }
      | undefined;
    if (
      !data ||
      typeof data.uri !== "string" ||
      typeof data.candidate !== "string" ||
      documents.get(data.uri)?.version !== data.version
    ) {
      return item;
    }
    const entries = await query(
      data.uri,
      {
        kind: "resolve",
        candidate: data.candidate,
      },
      token,
    );
    const answer = entries.find(
      (entry) =>
        entry.project === data.project &&
        entry.config === data.config &&
        entry.result.revision === data.revision,
    )?.result.hover;
    if (answer) {
      item.documentation = {
        kind: MarkupKind.Markdown,
        value: `\`\`\`css\n${answer.css}\n\`\`\``,
      };
    }
    return item;
  });
  connection.onDocumentColor(async (params, token) => {
    const document = documents.get(params.textDocument.uri);
    if (!document) {
      return [];
    }
    const entries = await query(
      document.uri,
      {
        kind: "colors",
      },
      token,
    );
    const colors = entries.flatMap((entry) => entry.result.colors ?? []);
    return colors
      .filter(
        (color, index) =>
          colors.findIndex((other) => JSON.stringify(other) === JSON.stringify(color)) === index &&
          !colors.some(
            (other) =>
              other.start === color.start &&
              other.end === color.end &&
              JSON.stringify(other) !== JSON.stringify(color),
          ),
      )
      .map((color) => ({
        range: offsets(document, color),
        color: {
          red: color.red,
          green: color.green,
          blue: color.blue,
          alpha: color.alpha,
        },
      }));
  });
  connection.onColorPresentation(async (params, token) => {
    const document = documents.get(params.textDocument.uri);
    if (!document) {
      return [];
    }
    const entries = await query(
      document.uri,
      {
        kind: "colorPresentation",
        start: document.offsetAt(params.range.start),
        end: document.offsetAt(params.range.end),
        color: params.color,
      },
      token,
    );
    return entries
      .flatMap((entry) =>
        (entry.result.edits ?? []).map((edit) => ({
          label: edit.newText,
          textEdit: {
            range: offsets(document, edit),
            newText: edit.newText,
          },
        })),
      )
      .filter(
        (item, index, items) =>
          items.findIndex((other) => JSON.stringify(item) === JSON.stringify(other)) === index,
      );
  });
}

/** Builds separate project actions; conflicting project edits are never combined into one action. */
export async function sourceActions(
  document: TextDocument,
  only: readonly string[] | undefined,
  token: CancellationToken,
  query: EditorQuery,
): Promise<CodeAction[]> {
  const actions: CodeAction[] = [];
  for (const [kind, operation, title] of [
    [fixAllKind, "fixes", "Fix all Stillwind lint findings"],
    [sortKind, "sort", "Sort Stillwind utilities"],
  ] as const) {
    if (!only?.some((requested) => kind === requested || kind.startsWith(`${requested}.`))) {
      continue;
    }
    const entries = await query(
      document.uri,
      {
        kind: operation,
      },
      token,
    );
    for (const entry of entries) {
      if (!entry.result.edits?.length) {
        continue;
      }
      const edit = {
        documentChanges: [
          {
            textDocument: {
              uri: document.uri,
              version: document.version,
            },
            edits: entry.result.edits.map((edit) => ({
              range: offsets(document, edit),
              newText: edit.newText,
            })),
          },
        ],
      };
      if (
        actions.some(
          (action) => action.kind === kind && JSON.stringify(action.edit) === JSON.stringify(edit),
        )
      ) {
        continue;
      }
      actions.push({
        title: `${title}${entries.length > 1 ? ` (${entry.config}, project ${entry.project + 1})` : ""}`,
        kind,
        edit,
      });
    }
  }
  return actions;
}

function offsets(
  document: TextDocument,
  range: {
    start: number;
    end: number;
  },
) {
  return {
    start: document.positionAt(range.start),
    end: document.positionAt(range.end),
  };
}
