import { resolve } from "node:path";

import * as vscode from "vscode";
import { LanguageClient, TransportKind } from "vscode-languageclient/node";

import { clientOptions } from "./client.ts";

let client: LanguageClient | undefined;

/**
 * Starts the bundled Node server independently of the Stillwind CLI.
 * @param context - Extension context that owns command registrations and file watching.
 * @returns Completion of restart command registration and LSP initialization.
 * @throws If server initialization fails.
 */
export async function activate(context: vscode.ExtensionContext): Promise<void> {
  const options = clientOptions(context);
  /**
   * Stops the current client and starts a Node server using the current configFile setting.
   *
   * @returns Completion of the replacement client's initialization.
   * @throws If the previous client fails to stop or the new client fails to start.
   */
  const start = async () => {
    await client?.stop();
    const config = vscode.workspace.getConfiguration("stillwind").get<string>("configFile");
    const server = {
      module: context.asAbsolutePath("dist/node/server.js"),
      transport: TransportKind.stdio,
      args: config
        ? [resolve(vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ?? "", config)]
        : [],
    };
    client = new LanguageClient("stillwind", "Stillwind", server, options);
    await client.start();
  };
  context.subscriptions.push(vscode.commands.registerCommand("stillwind.restart", start));
  await start();
}

/**
 * Stops the Node client and clears its reference after successful shutdown.
 *
 * @returns Completion of shutdown, including when no client is active.
 * @throws If the client fails to stop.
 */
export async function deactivate(): Promise<void> {
  await client?.stop();
  client = undefined;
}
