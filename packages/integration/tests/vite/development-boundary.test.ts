import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { stillwind } from "@stillwind/integration/vite";
import { createServer } from "vite";
import { expect, onTestFinished, test } from "vite-plus/test";

test.each(["css", "style.css"])(
  "the %s endpoint rejects an unknown environment",
  async (endpoint) => {
    const origin = await startDevelopmentServer();

    const response = await fetch(`${origin}/__stillwind/${endpoint}?environment=missing`);

    expect(response.status).toBe(404);
    expect(await response.json()).toEqual({ error: "Unknown environment" });
  },
);

test.each([
  { endpoint: "compile", method: "POST" },
  { endpoint: "preview?id=missing", method: "GET" },
  { endpoint: "preview-script?id=missing", method: "GET" },
])(
  "the Vite plugin does not serve $method /__stillwind/$endpoint",
  async ({ endpoint, method }) => {
    const origin = await startDevelopmentServer();

    const response = await fetch(`${origin}/__stillwind/${endpoint}`, { method });

    expect(response.status).toBe(404);
  },
);

async function startDevelopmentServer() {
  const root = await mkdtemp(path.join(tmpdir(), "stillwind-vite-boundary-"));
  const server = await createServer({
    configFile: false,
    root,
    appType: "custom",
    logLevel: "silent",
    plugins: [
      stillwind({
        css: fileURLToPath(new URL("../../../core/index.css", import.meta.url)),
        types: false,
      }),
    ],
    optimizeDeps: {
      noDiscovery: true,
      include: [],
    },
    server: {
      host: "127.0.0.1",
      port: 0,
      watch: null,
    },
  });
  onTestFinished(async () => {
    await server.close();
    await rm(root, { recursive: true, force: true });
  });
  await server.listen();
  const address = server.httpServer!.address();
  if (!address || typeof address === "string") {
    expect.fail("Expected Vite to listen on a TCP port");
  }
  return `http://127.0.0.1:${address.port}`;
}
