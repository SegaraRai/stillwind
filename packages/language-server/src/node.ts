import path from "node:path";
import process from "node:process";
import { pathToFileURL } from "node:url";

import { discoverNodeWorkspace, openNodeWorkspaces } from "@stillwind/analysis/node";
import { createConnection, ProposedFeatures } from "vscode-languageserver/node";

import { createLanguageServer } from "./server.ts";

/**
 * Starts a stdio LSP connection with isolated Node analysis processes.
 *
 * @param config - Explicit configuration path, resolved from the process working directory.
 * Omission discovers configurations from client workspace folders, or the working directory, and
 * from the documents that the client opens in them.
 * The process exits with code 0 after shutdown followed by exit, or code 1 after exit alone.
 */
export function startLanguageServer(config?: string): void {
  const connection = createConnection(ProposedFeatures.all, process.stdin, process.stdout);
  createLanguageServer(connection, {
    ...(config ? {} : { discoverWorkspace: discoverNodeWorkspace }),
    openWorkspaces: (context) => openNodeWorkspaces(context, config),
    configurationUri: (id) => (path.isAbsolute(id) ? pathToFileURL(id).href : undefined),
    onExit: (shutdown) => {
      process.exit(shutdown ? 0 : 1);
    },
  });
  connection.listen();
}
