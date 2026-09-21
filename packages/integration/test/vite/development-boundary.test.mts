import assert from "node:assert/strict";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { stillwind } from "@stillwind/integration/vite";
import { createServer } from "vite";

void test("the Vite adapter exposes CSS delivery without application preview endpoints", async () => {
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
    assert.ok(address && typeof address !== "string");
    const origin = `http://127.0.0.1:${address.port}`;
    const css = await fetch(`${origin}/__stillwind/css?environment=missing`);
    assert.equal(css.status, 404);
    assert.deepEqual(await css.json(), { error: "Unknown environment" });
    assert.equal((await fetch(`${origin}/__stillwind/compile`, { method: "POST" })).status, 404);
    for (const path of ["preview", "preview-script"]) {
      assert.equal((await fetch(`${origin}/__stillwind/${path}?id=missing`)).status, 404);
    }
  } finally {
    await server.close();
  }
});
