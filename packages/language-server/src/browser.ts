import {
  BrowserMessageReader,
  BrowserMessageWriter,
  createConnection,
  ProposedFeatures,
} from "vscode-languageserver/browser";

import { createLanguageServer, type LanguageServerOptions } from "./index.ts";

/**
 * Starts LSP message transport using analysis services supplied by the embedding application.
 *
 * @param port - Worker or MessagePort supported by the browser LSP message reader and writer.
 * @param options - Workspace factory and optional exit handler. The factory supplies browser
 * compiler and module services independently of the CLI.
 * @returns Cleanup for analysis services and the connection's reader and writer.
 */
export function startBrowserLanguageServer(
  port: ConstructorParameters<typeof BrowserMessageReader>[0],
  options: LanguageServerOptions,
): {
  /**
   * Closes analysis services and disposes transport resources even if service cleanup fails.
   *
   * @returns A promise that resolves after cleanup.
   * @throws If analysis service cleanup fails.
   */
  dispose(): Promise<void>;
} {
  const reader = new BrowserMessageReader(port);
  const writer = new BrowserMessageWriter(port);
  const connection = createConnection(ProposedFeatures.all, reader, writer);
  const server = createLanguageServer(connection, options);
  connection.listen();

  return {
    async dispose() {
      try {
        await server.dispose();
      } finally {
        connection.dispose();
        reader.dispose();
        writer.dispose();
      }
    },
  };
}
