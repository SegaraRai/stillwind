import assert from "node:assert/strict";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { createServer } from "vite-plus";

import { playground } from "../server/plugin.ts";

void test("Playground serves isolated compiled previews without the integration plugin", async () => {
  const server = await createServer({
    configFile: false,
    root: fileURLToPath(new URL("../", import.meta.url)),
    logLevel: "silent",
    plugins: [playground("styles/default-theme.css")],
    optimizeDeps: { noDiscovery: true, include: [] },
    server: { host: "127.0.0.1", port: 0 },
  });
  try {
    await server.listen();
    const address = server.httpServer!.address();
    assert.ok(address && typeof address !== "string");
    const origin = `http://127.0.0.1:${address.port}`;
    assert.equal((await fetch(`${origin}/__stillwind/compile`)).status, 405);
    const invalid = await fetch(`${origin}/__stillwind/compile`, {
      method: "POST",
      body: JSON.stringify({ source: 1, theme: "" }),
    });
    assert.equal(invalid.status, 400);
    const response = await fetch(`${origin}/__stillwind/compile`, {
      method: "POST",
      body: JSON.stringify({
        source:
          'import {style} from "stillwind/react";export function App(){return <div {...style("red")}>Preview</div>}',
        theme: "@utility red{color:red}@stillwind-slot utilities;",
      }),
    });
    assert.equal(response.status, 200);
    const output = (await response.json()) as {
      diagnostics: unknown[];
      previewUrl: string;
      css: string;
    };
    assert.deepEqual(output.diagnostics, []);
    assert.match(output.css, /color:red/);
    assert.match(output.previewUrl, /^\/__stillwind\/preview\?id=/);
    const preview = await fetch(origin + output.previewUrl);
    assert.equal(preview.status, 200);
    assert.match(await preview.text(), /Content-Security-Policy/);
    const script = await fetch(origin + output.previewUrl.replace("/preview?", "/preview-script?"));
    assert.equal(script.status, 200);
    assert.match(await script.text(), /Preview/);
    const second = await fetch(`${origin}/__stillwind/compile`, {
      method: "POST",
      body: JSON.stringify({
        source:
          'import {style} from "stillwind/react";function App(){return <div {...style("blue")}>Second</div>}export App;',
        theme: "@utility blue{color:blue}@stillwind-slot utilities;",
      }),
    });
    const next = (await second.json()) as typeof output;
    assert.deepEqual(next.diagnostics, []);
    assert.match(next.css, /color:blue/);
    assert.doesNotMatch(next.css, /color:red/);
    assert.notEqual(next.previewUrl, output.previewUrl);
    const retained = await fetch(
      origin + output.previewUrl.replace("/preview?", "/preview-script?"),
    );
    assert.match(await retained.text(), /Preview/);
    const missingApp = await fetch(`${origin}/__stillwind/compile`, {
      method: "POST",
      body: JSON.stringify({ source: "export const value = 1;", theme: "" }),
    });
    const failed = (await missingApp.json()) as {
      diagnostics: { code: string }[];
      previewUrl: string | null;
    };
    assert.equal(failed.previewUrl, null);
    assert.equal(failed.diagnostics[0]?.code, "SHPLAY001");
    assert.equal((await fetch(`${origin}/__stillwind/preview?id=missing`)).status, 404);
  } finally {
    await server.close();
  }
});
