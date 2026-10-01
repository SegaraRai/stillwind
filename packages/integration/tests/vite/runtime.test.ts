import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { stillwind } from "@stillwind/integration/vite";
import { createServer } from "vite-plus";
import { expect, test } from "vite-plus/test";

test.each([false, true])(
  "serves runtime entries under a base with devSourcemap=%s",
  async (sourceMaps) => {
    const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../../..");
    const root = mkdtempSync(path.join(repo, ".test-runtime-"));
    writeFileSync(path.join(root, "root.css"), ".base{color:red} @stillwind-slot utilities;");
    const server = await createServer({
      configFile: false,
      root,
      base: "/nested/",
      plugins: [stillwind({ css: path.join(root, "root.css"), types: false })],
      css: { devSourcemap: sourceMaps },
      optimizeDeps: { noDiscovery: true, include: [] },
      server: { host: "127.0.0.1", port: 0, watch: null },
      logLevel: "silent",
    });
    try {
      await server.listen();
      const address = server.httpServer!.address();
      if (!address || typeof address === "string") {
        expect.fail("expected HTTP address");
      }
      const origin = `http://127.0.0.1:${address.port}`;
      const response = await fetch(`${origin}/nested/@id/__x00__stillwind:runtime`);
      expect(response.status).toBe(200);
      const code = await response.text();
      const imports = [
        ...code.matchAll(/from\s+["']([^"']+\/runtime(?:-source-map)?\.js[^"']*)["']/g),
      ].map((match) => match[1]!);
      expect(imports).toHaveLength(sourceMaps ? 2 : 1);
      for (const url of imports) {
        const entry = await fetch(new URL(url, origin));
        expect(entry.status, url).toBe(200);
        expect(entry.headers.get("content-type"), url).toContain("javascript");
      }
      const snapshot = await fetch(`${origin}/nested/__stillwind/css?environment=client`);
      expect(snapshot.status).toBe(200);
      expect(await snapshot.json()).toMatchObject({
        parts: expect.arrayContaining([
          expect.objectContaining({ css: expect.stringContaining(".base") }),
        ]),
      });
      const ssr = await server.ssrLoadModule("virtual:stillwind/runtime");
      await ssr.synchronize(1);
    } finally {
      await server.close();
      rmSync(root, { recursive: true, force: true });
      Reflect.deleteProperty(globalThis, Symbol.for("stillwind.document"));
    }
  },
);
