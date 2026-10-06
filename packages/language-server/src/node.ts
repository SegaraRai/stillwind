import { openNodeWorkspaces } from "@stillwind/analysis/node";
import { createConnection, ProposedFeatures } from "vscode-languageserver/node";

import { createLanguageServer } from "./index.ts";

/** Starts the stdio transport with isolated Node analysis hosts. */
export function startLanguageServer(config?: string): void {
  const connection = createConnection(ProposedFeatures.all, process.stdin, process.stdout);
  createLanguageServer(connection, {
    openWorkspaces: (context) => openNodeWorkspaces(context, config),
    onExit: (shutdown) => {
      process.exit(shutdown ? 0 : 1);
    },
  });
  connection.listen();
}
