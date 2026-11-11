// Runs Vite for a generated application in the current directory, which resolves Vite from the
// installed dependencies of its e2e fixture. The fixture's `vite` package has no executable.
//
// Usage: `node vite.mjs build` or `node vite.mjs dev <port>`.

import { createRequire } from "node:module";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

const [mode, port] = process.argv.slice(2);
const root = process.cwd();
const require = createRequire(join(root, "noop.js"));
const vite = await import(pathToFileURL(require.resolve("vite")).href);

if (mode === "build") {
  await vite.build({ root });
} else if (mode === "dev") {
  const server = await vite.createServer({
    root,
    server: { port: Number(port), strictPort: true, host: "localhost" },
  });
  await server.listen();
  server.printUrls();
} else {
  throw new Error(`Unknown mode: ${mode}`);
}
