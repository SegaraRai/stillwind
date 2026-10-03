import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { stillwind } from "@stillwind/integration/vite";
import { createServer, type ViteDevServer } from "vite-plus";
import { expect, test } from "vite-plus/test";

test("the Vite adapter exposes CSS delivery without application preview endpoints", async () => {
  const root = await mkdtemp(path.join(tmpdir(), "stillwind-vite-boundary-"));
  let server: ViteDevServer | undefined;
  try {
    server = await createServer({
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
    await server.listen();
    const address = server.httpServer!.address();
    if (!(address && typeof address !== "string")) {
      expect.fail('Unexpected value: address && typeof address !== "string"');
    }
    const origin = `http://127.0.0.1:${address!.port}`;
    const css = await fetch(`${origin}/__stillwind/css?environment=missing`);
    expect(css.status).toBe(404);
    expect(await css.json()).toStrictEqual({ error: "Unknown environment" });
    const stylesheet = await fetch(`${origin}/__stillwind/style.css?environment=missing`);
    expect(stylesheet.status).toBe(404);
    expect(await stylesheet.json()).toStrictEqual({ error: "Unknown environment" });
    expect((await fetch(`${origin}/__stillwind/compile`, { method: "POST" })).status).toBe(404);
    for (const path of ["preview", "preview-script"]) {
      expect((await fetch(`${origin}/__stillwind/${path}?id=missing`)).status).toBe(404);
    }
  } finally {
    await server?.close();
    await rm(root, { recursive: true, force: true });
  }
});
