import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { extname, resolve, sep } from "node:path";

/** Serves a generated playground with the isolation headers required by WASM threads. */
export async function servePlayground(
  directory: string,
  transformJavaScript?: (source: string, file: string) => string,
) {
  const root = resolve(directory);
  const server = createServer(async (request, response) => {
    const file = resolve(
      root,
      "." +
        decodeURIComponent(new URL(request.url!, "http://localhost").pathname).replace(
          /\/$/,
          "/index.html",
        ),
    );
    if (!file.startsWith(root + sep)) {
      response.writeHead(403).end();
      return;
    }
    try {
      const extension = extname(file);
      const contents = await readFile(file);
      response
        .writeHead(200, {
          "Content-Type":
            (
              {
                ".html": "text/html",
                ".js": "text/javascript",
                ".mjs": "text/javascript",
                ".css": "text/css",
                ".json": "application/json",
                ".wasm": "application/wasm",
              } as Record<string, string>
            )[extension] ?? "application/octet-stream",
          "Cross-Origin-Opener-Policy": "same-origin",
          "Cross-Origin-Embedder-Policy": "require-corp",
        })
        .end(
          transformJavaScript && /\.[cm]?js$/.test(file)
            ? transformJavaScript(contents.toString("utf8"), file)
            : contents,
        );
    } catch {
      response.writeHead(404).end();
    }
  });
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const address = server.address();
  if (!address || typeof address === "string") {
    throw new Error("Missing playground server port");
  }
  return {
    origin: `http://127.0.0.1:${address.port}`,
    close: () =>
      new Promise<void>((resolve, reject) =>
        server.close((error) => (error ? reject(error) : resolve())),
      ),
  };
}
