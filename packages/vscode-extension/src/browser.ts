import * as vscode from "vscode";
import { LanguageClient } from "vscode-languageclient/browser";

import { clientOptions, readWorkspace } from "./client.ts";

let client: LanguageClient | undefined;
let worker: Worker | undefined;

/**
 * Starts the LSP worker with files read through the embedding's workspace provider.
 * @param context - Extension context that owns command registrations and file watching.
 * @returns Completion of command registration, workspace capture, and LSP initialization.
 * @throws If cross-origin isolation is unavailable or server initialization fails.
 */
export async function activate(context: vscode.ExtensionContext): Promise<void> {
  if (!globalThis.crossOriginIsolated || typeof SharedArrayBuffer === "undefined") {
    throw new Error("Stillwind Web requires cross-origin isolation (COOP and COEP headers)");
  }

  const options = clientOptions(context);
  /**
   * Stops the previous client, replaces its worker, and captures the current workspace for initialization.
   *
   * @returns Completion of the replacement client's initialization.
   * @throws If stopping, workspace capture, or client startup fails.
   */
  const start = async () => {
    await client?.stop();
    worker?.terminate();
    worker = new Worker(
      vscode.Uri.joinPath(context.extensionUri, "dist/browser/server.js").toString(true),
      { name: "Stillwind language server" },
    );
    worker.postMessage({
      bootstrap: vscode.Uri.joinPath(
        context.extensionUri,
        "dist/browser/server-module.js",
      ).toString(true),
    });
    client = new LanguageClient("stillwind", "Stillwind", worker, {
      ...options,
      initializationOptions: {
        files: await readWorkspace(
          vscode.workspace.workspaceFolders?.map((folder) => folder.uri.toString()) ?? [],
        ),
        configFile:
          vscode.workspace.getConfiguration("stillwind").get<string>("configFile") || undefined,
      },
    });
    client.onRequest(
      "stillwind/readWorkspace",
      async ({
        roots,
      }: {
        /** Workspace directory URIs requested by the server during reload. */
        roots: string[];
      }) => ({
        files: await readWorkspace(roots),
        configFile:
          vscode.workspace.getConfiguration("stillwind").get<string>("configFile") || undefined,
      }),
    );
    await client.start();
  };

  context.subscriptions.push(
    vscode.commands.registerCommand("stillwind.restart", start),
    vscode.commands.registerCommand("stillwind.snapshot", async (input?: unknown) => {
      return client?.sendRequest("stillwind/snapshot", input);
    }),
    vscode.commands.registerCommand("stillwind.compile", async (input: unknown) => {
      return client?.sendRequest("stillwind/compile", input);
    }),
  );

  await start();
}

/**
 * Stops the LSP and releases its worker and stored references even if client shutdown fails.
 *
 * @returns Completion of client shutdown and worker termination.
 * @throws If the client fails to stop, after worker cleanup.
 */
export async function deactivate(): Promise<void> {
  try {
    await client?.stop();
  } finally {
    worker?.terminate();
    client = undefined;
    worker = undefined;
  }
}
