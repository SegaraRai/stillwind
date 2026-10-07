import type { Options } from "prettier";
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

/** Identity of the embedded extension that owns the document formatter provider. */
export const formatterManifest = {
  name: "prettier",
  publisher: "stillwind-internal",
  displayName: "Prettier (Playground)",
  version: "0.0.0",
  engines: {
    vscode: "*",
  },
};

/** Language-specific defaults leave unsupported files with their existing formatter. */
export const formatterConfiguration = {
  [Object.keys(parsers)
    .map((language) => `[${language}]`)
    .join("")]: {
    "editor.defaultFormatter": `${formatterManifest.publisher}.${formatterManifest.name}`,
  },
};

/**
 * Registers full-document formatting with the embedded formatter extension's API.
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
      const formatted = await formatSource(text, document.languageId, options);
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
 * @param text - Complete original source text.
 * @param language - VS Code language ID; unsupported IDs return the text unchanged.
 * @param options - Editor indentation preferences. Other formatting options use Prettier defaults.
 * @returns Formatted source. Stillwind utility strings retain their authored token order.
 * @throws If a parser cannot parse the source or a bundled plugin cannot load.
 */
export async function formatSource(
  text: string,
  language: string,
  options: Pick<vscode.FormattingOptions, "insertSpaces" | "tabSize">,
): Promise<string> {
  if (!Object.hasOwn(parsers, language)) {
    return text;
  }
  const parser = parsers[language as keyof typeof parsers];
  const { format } = await import("prettier/standalone");
  let plugins: Options["plugins"];
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
  return format(text, {
    parser,
    plugins,
    useTabs: !options.insertSpaces,
    tabWidth: options.tabSize,
  });
}
