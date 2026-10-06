import {
  BrowserMessageReader,
  BrowserMessageWriter,
  createConnection,
  ProposedFeatures,
} from "vscode-languageserver/browser";

import { createLanguageServer, type LanguageServerOptions } from "./index.ts";

/** Starts LSP on a browser Worker or MessagePort using caller-supplied analysis services. */
export function startBrowserLanguageServer(
  port: ConstructorParameters<typeof BrowserMessageReader>[0],
  options: LanguageServerOptions,
): {
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
