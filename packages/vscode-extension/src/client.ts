import * as vscode from "vscode";
import type { LanguageClientOptions } from "vscode-languageclient";

/**
 * Creates document selection and file watching, and registers the utility-sort command.
 *
 * @param context - Receives the watcher and command registrations for the extension's lifetime.
 * @returns Options that synchronize source, theme, and configuration buffers.
 */
export function clientOptions(context: vscode.ExtensionContext): LanguageClientOptions {
  const watcher = vscode.workspace.createFileSystemWatcher("**/*");

  context.subscriptions.push(
    watcher,
    vscode.commands.registerCommand("stillwind.sortUtilities", async () => {
      const editor = vscode.window.activeTextEditor;
      if (!editor) {
        vscode.window.setStatusBarMessage("Stillwind: No active editor to sort", 3000);
        return;
      }
      const { document } = editor;
      const actions =
        (await vscode.commands.executeCommand<vscode.CodeAction[]>(
          "vscode.executeCodeActionProvider",
          document.uri,
          new vscode.Range(0, 0, document.lineCount, 0),
          "source.sort.stillwind",
        )) ?? [];
      const action = actions.find(
        (action) => action.kind?.value === "source.sort.stillwind" && action.edit,
      );
      if (!action?.edit) {
        vscode.window.setStatusBarMessage("Stillwind: Utilities are already sorted", 3000);
        return;
      }
      await vscode.workspace.applyEdit(action.edit);
    }),
  );

  return {
    initializationOptions: editorSettings,
    // The client pulls documents in open tabs, so documents that extensions open in the background
    // start no analysis.
    diagnosticPullOptions: {
      onChange: true,
      onSave: true,
      onFocus: true,
      onTabs: false,
    },
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
      configurationSection: "stillwind",
      fileEvents: watcher,
    },
  };
}

/**
 * Shows which language server runs in a status bar item that opens the Stillwind output.
 *
 * @param context - Receives the item and its command registration for the extension's lifetime.
 * @param output - Output channel that the item opens.
 * @returns A function that replaces the item's state. `starting` shows progress, `running` shows the
 * optional `server` sentence in the tooltip, and `failed` shows an error icon.
 */
export function serverStatus(
  context: vscode.ExtensionContext,
  output: vscode.OutputChannel,
): (state: "starting" | "running" | "failed", server?: string) => void {
  const item = vscode.window.createStatusBarItem(
    "stillwind.server",
    vscode.StatusBarAlignment.Right,
  );
  item.name = "Stillwind";
  item.command = "stillwind.showOutput";
  context.subscriptions.push(
    item,
    vscode.commands.registerCommand("stillwind.showOutput", () => output.show(true)),
  );
  return (state, server) => {
    item.text = {
      starting: "$(sync~spin) Stillwind",
      running: "$(check) Stillwind",
      failed: "$(error) Stillwind",
    }[state];
    item.tooltip = {
      starting: "Stillwind: Starting the language server",
      running: `Stillwind: ${server ?? "The language server is running."} Click to show the output.`,
      failed: "Stillwind: The language server failed to start. Click to show the output.",
    }[state];
    item.show();
  };
}

/**
 * Restarts the language server when `stillwind.configFile` changes, because the server receives
 * the configuration path only when it starts.
 *
 * @param context - Receives the listener for the extension's lifetime.
 * @param restart - Starts a replacement client for the current setting.
 */
export function restartOnConfigFileChange(
  context: vscode.ExtensionContext,
  restart: () => Promise<void>,
): void {
  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration((event) => {
      if (event.affectsConfiguration("stillwind.configFile")) {
        // The status bar and the client's output channel report a failed restart.
        restart().catch(() => undefined);
      }
    }),
  );
}

/**
 * Reads completion discovery preferences independently of project compiler configuration.
 *
 * @returns Initialization fields shared by the Node client and the browser client.
 */
export function editorSettings() {
  return {
    completion: {
      registryDirectives:
        vscode.workspace
          .getConfiguration("stillwind")
          .get<boolean>("completion.registryDirectives") === true,
    },
  };
}

/**
 * Captures text files through VS Code's filesystem provider and overlays open buffers.
 *
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
