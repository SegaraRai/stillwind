import { fileURLToPath } from "node:url";

import { stillwind } from "@stillwind/integration/vite";
import { createServer } from "vite";
import { expect, test } from "vite-plus/test";

test("the Vite adapter exposes CSS delivery without application preview endpoints", async () => {
  const server = await createServer({
    configFile: false,
    appType: "custom",
    logLevel: "silent",
    plugins: [
      stillwind({
        css: fileURLToPath(new URL("../../../core/index.css", import.meta.url)),
        types: false,
      }),
    ],
    optimizeDeps: { noDiscovery: true, include: [] },
    server: { host: "127.0.0.1", port: 0 },
  });
  try {
    await server.listen();
    const address = server.httpServer!.address();
    if (!(address && typeof address !== "string")) {
      expect.fail('Unexpected value: address && typeof address !== "string"');
    }
    const origin = `http://127.0.0.1:${address!.port}`;
    const css = await fetch(`${origin}/__stillwind/css?environment=missing`);
    expect(css.status).toBe(404);
    expect(await css.json()).toStrictEqual({ error: "Unknown environment" });
    expect((await fetch(`${origin}/__stillwind/compile`, { method: "POST" })).status).toBe(404);
    for (const path of ["preview", "preview-script"]) {
      expect((await fetch(`${origin}/__stillwind/${path}?id=missing`)).status).toBe(404);
    }
  } finally {
    await server.close();
  }
});
