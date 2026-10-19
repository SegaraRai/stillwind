import type { EditorRequest, WorkspaceEditorResult } from "@stillwind/analysis";
import {
  CompletionItemKind,
  InsertTextFormat,
  MarkupKind,
  type CancellationToken,
  type CodeAction,
  type CompletionItem,
  type Connection,
  type TextDocuments,
} from "vscode-languageserver";
import type { TextDocument } from "vscode-languageserver-textdocument";

const sortKind = "source.sort.stillwind";
const fixAllKind = "source.fixAll.stillwind";

/** Source actions advertised in fix-all order followed by utility sorting. */
export const editorCodeActionKinds = [fixAllKind, sortKind];

/**
 * Queries the current original buffer and discards results after a document change or cancellation.
 *
 * @param uri - URI of an open original document.
 * @param request - Editor operation with offsets in that document's UTF-16 text.
 * @param token - Cancellation for this request.
 * @returns Results whose source fingerprints match the current document.
 */
export type EditorQuery = (
  uri: string,
  request: EditorRequest,
  token: CancellationToken,
) => Promise<WorkspaceEditorResult[]>;

/**
 * Registers hover, completion, colors, plugin links, and source actions on the connection.
 *
 * @param connection - Caller-owned connection on which handlers are installed.
 * @param documents - Open original buffers used to convert UTF-16 offsets to LSP positions.
 * @param query - Query implementation that rejects obsolete results and honors cancellation.
 */
export function registerEditorFeatures(
  connection: Connection,
  documents: TextDocuments<TextDocument>,
  query: EditorQuery,
): void {
  connection.onDocumentLinks(async (params, token) => {
    const document = documents.get(params.textDocument.uri);
    if (!document) {
      return [];
    }
    const entries = await query(
      document.uri,
      {
        kind: "links",
      },
      token,
    );
    const links = entries.flatMap((entry) => entry.result.links ?? []);
    return links
      .filter(
        (link, index) =>
          link.target &&
          links.findIndex(
            (other) =>
              other.start === link.start && other.end === link.end && other.target === link.target,
          ) === index &&
          !links.some(
            (other) =>
              other.start === link.start && other.end === link.end && other.target !== link.target,
          ),
      )
      .map((link) => ({
        range: offsets(document, link),
        target: link.target,
      }));
  });
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
        const hover = entry.result.hover!;
        const key = JSON.stringify(hover);
        if (seen.has(key)) {
          return [];
        }
        seen.add(key);
        return [
          `${answers.length > 1 ? `${entry.config} · project ${entry.project + 1}\n\n` : ""}${hoverMarkdown(hover)}`,
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
        detail: item.detail,
        sortText: item.sortText,
        insertTextFormat: item.snippet ? InsertTextFormat.Snippet : undefined,
        command: item.retrigger
          ? { title: "Continue completion", command: "editor.action.triggerSuggest" }
          : undefined,
        kind: item.keyword
          ? CompletionItemKind.Keyword
          : item.category === "directory"
            ? CompletionItemKind.Folder
            : item.category === "file"
              ? CompletionItemKind.File
              : item.category === "module" || item.variant || item.candidate.endsWith(":")
                ? CompletionItemKind.Module
                : item.marker
                  ? CompletionItemKind.Class
                  : CompletionItemKind.Value,
        textEdit: {
          range: offsets(document, item),
          newText: item.snippet ?? item.newText,
        },
        data: {
          uri: document.uri,
          version,
          revision: entry.result.revision,
          candidate: item.candidate,
          input: item.input,
          snapshot: item.snapshot,
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
    const detail = item.detail;
    delete item.documentation;
    delete item.detail;
    const data = item.data as
      | {
          /** Original document URI captured when completion was requested. */
          uri?: unknown;
          /** Document version that must still match the open buffer. */
          version?: unknown;
          /** Full completion candidate, including surrounding variants. */
          candidate?: unknown;
          /** Configuration identity used to select the same project result. */
          config?: unknown;
          /** Zero-based project index within the configuration. */
          project?: unknown;
          /** Source fingerprint that must match the resolved result. */
          revision?: unknown;
          /** Original UTF-16 offset that identifies the completion's utility input. */
          input?: unknown;
          /** Graph and configuration fingerprint required by the resolve request. */
          snapshot?: unknown;
        }
      | undefined;
    if (
      !data ||
      typeof data.uri !== "string" ||
      typeof data.candidate !== "string" ||
      typeof data.input !== "number" ||
      typeof data.snapshot !== "string" ||
      documents.get(data.uri)?.version !== data.version
    ) {
      return item;
    }

    const entries = await query(
      data.uri,
      {
        kind: "resolve",
        candidate: data.candidate,
        input: data.input,
        snapshot: data.snapshot,
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
      item.detail = answer.detail ?? detail;
      item.documentation = {
        kind: MarkupKind.Markdown,
        value: hoverMarkdown(answer),
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

/**
 * Formats CSS previews and dialect documentation, and explains authored runtime key expressions.
 * Expression line breaks become spaces, and code-span delimiters exceed every authored
 * backtick run so expressions remain literal Markdown. The supplied hover remains unchanged.
 *
 * @param hover - CSS preview and optional authored expressions for runtime view-transition names.
 * @returns Markdown with CSS first and expression notes in their supplied order.
 */
function hoverMarkdown(hover: {
  /** Formatted generated CSS included verbatim inside the CSS fence. */
  css: string;
  /** Markdown describing authored CSS syntax or theme semantics. */
  documentation?: string;
  /** Authored expressions in the order in which their notes should appear. */
  runtimeNames?: string[];
}): string {
  const notes = (hover.runtimeNames ?? []).map((expression) => {
    const text = expression.replace(/\r\n?|\n/g, " ");
    const ticks = "`".repeat(
      Math.max(0, ...[...text.matchAll(/`+/g)].map((match) => match[0].length)) + 1,
    );
    return `view-transition-name is computed at runtime from ${ticks} ${text} ${ticks}.`;
  });
  return [hover.css ? `\`\`\`css\n${hover.css}\n\`\`\`` : undefined, hover.documentation, ...notes]
    .filter(Boolean)
    .join("\n\n");
}

/**
 * Builds versioned source actions for each project and removes actions with identical edits.
 * Each action contains one project's edits, so differing project results remain separate.
 *
 * @param document - Open original buffer that the query reads and the returned edits target.
 * @param only - Requested source kinds or their prefixes. Omission or an empty list produces no actions.
 * @param token - Cancellation forwarded to each query.
 * @param query - Query implementation that discards changed buffers and cancelled requests.
 * @returns Fix-all actions followed by sort actions, preserving project order within each kind.
 * @throws If a query rejects.
 */
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

/**
 * Converts original UTF-16 offsets to positions using the open document's line table.
 *
 * @param document - Document containing the text from which the offsets were measured.
 * @param range - Half-open UTF-16 range. TextDocument clamps offsets outside the document.
 * @returns LSP positions with zero-based lines and UTF-16 character offsets.
 */
function offsets(
  document: TextDocument,
  range: {
    /** Inclusive UTF-16 offset in the original document. */
    start: number;
    /** Exclusive UTF-16 offset in the original document. */
    end: number;
  },
) {
  return {
    start: document.positionAt(range.start),
    end: document.positionAt(range.end),
  };
}
