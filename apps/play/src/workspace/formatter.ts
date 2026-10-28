import type { Options } from "prettier";
import type { SortInput, SortOutput } from "prettier-plugin-stillwind/browser";
import type * as vscode from "vscode";

const parsers = {
  javascript: "babel",
  javascriptreact: "babel",
  typescript: "typescript",
  typescriptreact: "typescript",
  css: "css",
  json: "json",
  jsonc: "jsonc",
} as const;

/**
 * Identity of the embedded extension that owns the document formatter provider.
 */
export const formatterManifest = {
  name: "prettier",
  publisher: "stillwind-internal",
  displayName: "Prettier (Playground)",
  version: "0.0.0",
  engines: {
    vscode: "*",
  },
};

/**
 * Language-specific defaults leave unsupported files with their existing formatter.
 */
export const formatterConfiguration = {
  [Object.keys(parsers)
    .map((language) => `[${language}]`)
    .join("")]: {
    "editor.defaultFormatter": `${formatterManifest.publisher}.${formatterManifest.name}`,
  },
};

/**
 * Stillwind sorting for one document, supplied to the Prettier plugin.
 */
export interface SourceSorting {
  /** Absolute virtual path that selects the parser wrapper and the Stillwind project. */
  filepath: string;
  /**
   * Sorts the original buffer and leaves its protected ranges untouched.
   *
   * @throws If the language server rejects the buffer or its projects disagree.
   */
  sort(input: SortInput): Promise<SortOutput>;
}

/**
 * Registers full-document formatting with the embedded formatter extension's API.
 * Stillwind sorting runs inside Prettier through `prettier-plugin-stillwind`, and the Web
 * extension's `stillwind.sortSource` command computes the sort edits.
 *
 * @param api - API scoped to the formatter extension, so VS Code can select it by extension ID.
 * @returns Registration to dispose with the playground workspace. Cancelled requests and changed
 * buffers produce no edits; syntax errors are reported through VS Code's formatter error handling.
 */
export function registerFormatter(api: typeof vscode): vscode.Disposable {
  return api.languages.registerDocumentFormattingEditProvider(Object.keys(parsers), {
    async provideDocumentFormattingEdits(document, options, token) {
      if (token.isCancellationRequested) {
        return [];
      }
      const text = document.getText();
      const version = document.version;
      const formatted = await formatSource(text, document.languageId, options, {
        filepath: document.uri.path,
        async sort(input) {
          const result = await api.commands.executeCommand<
            { edits: SortOutput["edits"] } | undefined
          >("stillwind.sortSource", { uri: document.uri.toString(), ...input });
          const edits = result?.edits ?? [];
          return { code: applyEdits(input.source, edits), edits };
        },
      });
      if (token.isCancellationRequested || document.version !== version || formatted === text) {
        return [];
      }
      return [
        api.TextEdit.replace(
          new api.Range(document.positionAt(0), document.positionAt(text.length)),
          formatted,
        ),
      ];
    },
  });
}

/**
 * Formats one buffer with Prettier's browser runtime and bundled parser plugins.
 *
 * @param text - Complete original source text.
 * @param language - VS Code language ID; unsupported IDs return the text unchanged.
 * @param options - Editor indentation preferences. Other formatting options use Prettier defaults.
 * @param sorting - Stillwind sorting for JS, TS, and CSS sources. Omission keeps the authored
 *   utility order.
 * @returns Formatted source.
 * @throws If a parser cannot parse the source, a bundled plugin cannot load, or sorting fails.
 */
export async function formatSource(
  text: string,
  language: string,
  options: Pick<vscode.FormattingOptions, "insertSpaces" | "tabSize">,
  sorting?: SourceSorting,
): Promise<string> {
  if (!Object.hasOwn(parsers, language)) {
    return text;
  }
  const parser = parsers[language as keyof typeof parsers];
  const { format } = await import("prettier/standalone");
  let plugins: NonNullable<Options["plugins"]>;
  if (parser === "css") {
    plugins = [await import("prettier/plugins/postcss")];
  } else {
    plugins = [
      await import("prettier/plugins/estree"),
      await (parser === "typescript"
        ? import("prettier/plugins/typescript")
        : import("prettier/plugins/babel")),
    ];
  }
  if (sorting && parser !== "json" && parser !== "jsonc") {
    const { createStillwindPlugin } = await import("prettier-plugin-stillwind/browser");
    plugins.push(createStillwindPlugin(() => (input) => sorting.sort(input)));
  }
  return format(text, {
    parser,
    plugins,
    filepath: sorting?.filepath,
    useTabs: !options.insertSpaces,
    tabWidth: options.tabSize,
  });
}

function applyEdits(source: string, edits: SortOutput["edits"]): string {
  let code = source;
  for (const edit of [...edits].sort((a, b) => b.start - a.start)) {
    code = code.slice(0, edit.start) + edit.newText + code.slice(edit.end);
  }
  return code;
}
