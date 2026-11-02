import * as vscode from "vscode";
import { LanguageClient } from "vscode-languageclient/browser";

import { clientOptions, editorSettings, readWorkspace } from "./client.ts";

let client: LanguageClient | undefined;
let worker: Worker | undefined;

/**
 * Starts the LSP worker with files read through the embedding's workspace provider.
 *
 * @param context - Extension context that owns command registrations, file watching, and the output
 * channel.
 * @returns Completion of command registration, workspace capture, and LSP initialization.
 * @throws If cross-origin isolation is unavailable, after the error is logged and shown, or if
 * server initialization fails.
 */
export async function activate(context: vscode.ExtensionContext): Promise<void> {
  const output = vscode.window.createOutputChannel("Stillwind", {
    log: true,
  });
  context.subscriptions.push(output);
  output.info(
    `Stillwind extension ${(context.extension.packageJSON as { version: string }).version}`,
  );
  if (!globalThis.crossOriginIsolated || typeof SharedArrayBuffer === "undefined") {
    const message = "Stillwind Web requires cross-origin isolation (COOP and COEP headers)";
    output.error(message);
    void vscode.window.showErrorMessage(message);
    throw new Error(message);
  }

  const options = {
    ...clientOptions(context),
    outputChannel: output,
  };
  /**
   * Captures workspace files and logs their count, the capture time, and the configuration setting.
   *
   * @param roots - Workspace directory URIs.
   * @returns Captured files and the explicit configuration path, if the setting has one.
   * @throws If workspace capture fails.
   */
  const capture = async (roots: readonly string[]) => {
    const start = performance.now();
    const files = await readWorkspace(roots);
    output.info(
      `Captured ${files.length} workspace files in ${Math.round(performance.now() - start)} ms`,
    );
    const configFile =
      vscode.workspace.getConfiguration("stillwind").get<string>("configFile") || undefined;
    if (configFile) {
      output.info(`Using the configuration file ${configFile} from stillwind.configFile`);
    }
    return {
      files,
      configFile,
    };
  };
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
    const initializationOptions = {
      ...editorSettings(),
      ...(await capture(
        vscode.workspace.workspaceFolders?.map((folder) => folder.uri.toString()) ?? [],
      )),
    };
    client = new LanguageClient("stillwind", "Stillwind", worker, {
      ...options,
      initializationOptions,
    });
    client.onRequest(
      "stillwind/readWorkspace",
      async ({
        roots,
      }: {
        /** Workspace directory URIs requested by the server during reload. */
        roots: string[];
      }) => capture(roots),
    );
    try {
      await client.start();
    } finally {
      initializationOptions.files = [];
    }
  };

  context.subscriptions.push(
    vscode.commands.registerCommand("stillwind.restart", start),
    vscode.commands.registerCommand("stillwind.snapshot", async (input?: unknown) => {
      return client?.sendRequest("stillwind/snapshot", input);
    }),
    vscode.commands.registerCommand("stillwind.compile", async (input: unknown) => {
      return client?.sendRequest("stillwind/compile", input);
    }),
    vscode.commands.registerCommand("stillwind.readFile", async (input: unknown) => {
      return client?.sendRequest("stillwind/readFile", input);
    }),
    vscode.commands.registerCommand("stillwind.sortSource", async (input: unknown) => {
      return client?.sendRequest("stillwind/sort", input);
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
