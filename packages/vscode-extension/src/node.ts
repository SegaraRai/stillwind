import { resolve } from "node:path";

import * as vscode from "vscode";
import { LanguageClient, TransportKind } from "vscode-languageclient/node";

import { clientOptions } from "./client.ts";

let client: LanguageClient | undefined;

/**
 * Starts the bundled Node server independently of the Stillwind CLI.
 * @param context - Extension context that owns command registrations and file watching.
 * @throws If server initialization fails.
 */
export async function activate(context: vscode.ExtensionContext): Promise<void> {
  const options = clientOptions(context);
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

/** Stops the Node server during extension shutdown. */
export async function deactivate(): Promise<void> {
  await client?.stop();
  client = undefined;
}
