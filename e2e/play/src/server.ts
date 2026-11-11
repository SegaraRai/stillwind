import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { extname, resolve, sep } from "node:path";

/**
 * Returns the response headers that the deployment sends for every file of a generated playground:
 * the isolation headers required by WASM threads and the Content Security Policy from the
 * build's `_headers` file. Throws when `_headers` sets no policy for every path.
 */
export async function playgroundHeaders(directory: string): Promise<Record<string, string>> {
  const headers = await readFile(resolve(directory, "_headers"), "utf8");
  const policy = /^\/\*\r?\n(?: {2}.*\r?\n)*? {2}Content-Security-Policy: (.+)$/m.exec(
    headers,
  )?.[1];
  if (!policy) {
    throw new Error(`${directory}/_headers sets no Content-Security-Policy for /*`);
  }
  return {
    "Cross-Origin-Opener-Policy": "same-origin",
    "Cross-Origin-Embedder-Policy": "require-corp",
    "Content-Security-Policy": policy.trim(),
  };
}

/** Serves a generated playground with the headers of its deployment. */
export async function servePlayground(
  directory: string,
  transformJavaScript?: (source: string, file: string) => string,
) {
  const root = resolve(directory);
  const headers = await playgroundHeaders(root);
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
          ...headers,
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
