import * as vscode from "vscode";
import type { LanguageClientOptions } from "vscode-languageclient";

/**
 * Creates document selection and file watching, and registers the utility-sort command.
 * @param context - Receives the watcher and command registrations for the extension's lifetime.
 * @returns Options that synchronize source, theme, and configuration buffers.
 */
export function clientOptions(context: vscode.ExtensionContext): LanguageClientOptions {
  const watcher = vscode.workspace.createFileSystemWatcher("**/*");

  context.subscriptions.push(
    watcher,
    vscode.commands.registerCommand("stillwind.sortUtilities", () =>
      vscode.commands.executeCommand("editor.action.codeAction", {
        kind: "source.sort.stillwind",
        apply: "ifSingle",
      }),
    ),
  );

  return {
    documentSelector: [
      "javascript",
      "javascriptreact",
      "typescript",
      "typescriptreact",
      "css",
      "json",
      "jsonc",
      "astro",
      "svelte",
      "vue",
    ].map((language) => ({ language })),
    synchronize: {
      fileEvents: watcher,
    },
  };
}

/**
 * Captures text files through VS Code's filesystem provider and overlays open buffers.
 * @param roots - Workspace directory URIs. Symlinks, .git, and Rust target directories are skipped.
 * Files with supported lowercase extensions are decoded as UTF-8. Open buffers beneath the roots
 * replace captured text or add entries regardless of their extension.
 * @returns Unique URI/text pairs in traversal order, with newly added open buffers appended.
 * @throws If the provider cannot enumerate or read a file.
 */
export async function readWorkspace(roots: readonly string[]): Promise<[string, string][]> {
  const files: [string, string][] = [];
  /**
   * Appends supported text files by recursively reading a provider directory.
   *
   * @param directory - Directory URI supported by VS Code's filesystem provider.
   * @returns Completion of traversal, skipping symlinks, .git, and target directories.
   * @throws If a directory or selected file cannot be read.
   */
  const read = async (directory: vscode.Uri) => {
    for (const [name, kind] of await vscode.workspace.fs.readDirectory(directory)) {
      if (name === ".git" || name === "target") {
        continue;
      }
      const uri = vscode.Uri.joinPath(directory, name);
      if (kind & vscode.FileType.SymbolicLink) {
        continue;
      }
      if (kind & vscode.FileType.Directory) {
        await read(uri);
      } else if (/\.(?:[cm]?[jt]sx?|json|css|svg|vue|svelte|astro)$/.test(name)) {
        files.push([
          uri.toString(),
          new TextDecoder().decode(await vscode.workspace.fs.readFile(uri)),
        ]);
      }
    }
  };
  for (const root of roots) {
    await read(vscode.Uri.parse(root));
  }

  const captured = new Map(files);
  for (const document of vscode.workspace.textDocuments) {
    if (roots.some((root) => document.uri.toString().startsWith(root.replace(/\/$/, "") + "/"))) {
      captured.set(document.uri.toString(), document.getText());
    }
  }
  return [...captured];
}
