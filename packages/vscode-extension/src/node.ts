import { dirname, resolve } from "node:path";

import * as vscode from "vscode";
import { LanguageClient, TransportKind } from "vscode-languageclient/node";

import { clientOptions, reportVisibleDocuments } from "./client.ts";
import { cliServer, findProjectCli, serverEnvironment } from "./project-cli.ts";

let client: LanguageClient | undefined;

/**
 * Starts the project's Stillwind CLI, or the bundled Node server when the project has no usable CLI.
 *
 * @param context - Extension context that owns command registrations, file watching, and the output
 * channel.
 * @returns Completion of restart command registration and LSP initialization.
 * @throws If the bundled server fails to initialize.
 */
export async function activate(context: vscode.ExtensionContext): Promise<void> {
  const output = vscode.window.createOutputChannel("Stillwind", {
    log: true,
  });
  context.subscriptions.push(output);
  const options = {
    ...clientOptions(context),
    outputChannel: output,
  };
  /**
   * Stops the current client and starts one client for the current setting and project.
   *
   * @returns Completion of the replacement client's initialization.
   * @throws If the previous client fails to stop or the bundled server fails to start.
   */
  const start = async () => {
    await client?.stop();
    client = undefined;
    const folders = (vscode.workspace.workspaceFolders ?? [])
      .filter((folder) => folder.uri.scheme === "file")
      .map((folder) => folder.uri.fsPath);
    const setting = vscode.workspace.getConfiguration("stillwind").get<string>("configFile");
    const config = setting ? resolve(folders[0] ?? "", setting) : undefined;
    const cli = findProjectCli(config ? [dirname(config), ...folders] : folders);
    if (cli) {
      output.info(`Starting @stillwind/cli ${cli.version} from ${cli.module}`);
      const projectClient = new LanguageClient(
        "stillwind",
        "Stillwind",
        cliServer(cli, config),
        options,
      );
      try {
        await projectClient.start();
        client = projectClient;
        return;
      } catch (error) {
        output.warn(`The project CLI failed to start: ${String(error)}`);
        await projectClient.dispose().catch(() => undefined);
      }
    }
    output.info("Starting the bundled language server");
    client = new LanguageClient(
      "stillwind",
      "Stillwind",
      {
        module: context.asAbsolutePath("dist/node/server.js"),
        transport: TransportKind.stdio,
        args: config ? ["--config", config] : [],
        options: {
          env: serverEnvironment(),
        },
      },
      options,
    );
    await client.start();
  };
  context.subscriptions.push(vscode.commands.registerCommand("stillwind.restart", start));
  reportVisibleDocuments(context, () => client);
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
