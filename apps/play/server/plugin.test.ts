import { fileURLToPath } from "node:url";

import { createServer } from "vite-plus";
import { expect, test } from "vite-plus/test";

import { playground } from "./plugin.ts";

test("Playground serves isolated compiled previews without the integration plugin", async () => {
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
    if (!address || typeof address === "string") {
      expect.fail("expected the server to listen on a TCP port");
    }
    const origin = `http://127.0.0.1:${address!.port}`;
    expect((await fetch(`${origin}/__stillwind/compile`)).status).toBe(405);
    const invalid = await fetch(`${origin}/__stillwind/compile`, {
      method: "POST",
      body: JSON.stringify({ source: 1, theme: "" }),
    });
    expect(invalid.status).toBe(400);
    const response = await fetch(`${origin}/__stillwind/compile`, {
      method: "POST",
      body: JSON.stringify({
        source:
          'import {style} from "stillwind/react";export function App(){return <div {...style("red")}>Preview</div>}',
        theme: "@utility red{color:red}@stillwind-slot utilities;",
      }),
    });
    expect(response.status).toBe(200);
    const output = (await response.json()) as {
      diagnostics: unknown[];
      previewUrl: string;
      css: string;
    };
    expect(output.diagnostics).toStrictEqual([]);
    expect(output.css).toMatch(/color:red/);
    expect(output.previewUrl).toMatch(/^\/__stillwind\/preview\?id=/);
    const preview = await fetch(origin + output.previewUrl);
    expect(preview.status).toBe(200);
    expect(await preview.text()).toMatch(/Content-Security-Policy/);
    const script = await fetch(origin + output.previewUrl.replace("/preview?", "/preview-script?"));
    expect(script.status).toBe(200);
    expect(await script.text()).toMatch(/Preview/);
    const second = await fetch(`${origin}/__stillwind/compile`, {
      method: "POST",
      body: JSON.stringify({
        source:
          'import {style} from "stillwind/react";function App(){return <div {...style("blue")}>Second</div>}export App;',
        theme: "@utility blue{color:blue}@stillwind-slot utilities;",
      }),
    });
    const next = (await second.json()) as typeof output;
    expect(next.diagnostics).toStrictEqual([]);
    expect(next.css).toMatch(/color:blue/);
    expect(next.css).not.toMatch(/color:red/);
    expect(next.previewUrl).not.toBe(output.previewUrl);
    const retained = await fetch(
      origin + output.previewUrl.replace("/preview?", "/preview-script?"),
    );
    expect(await retained.text()).toMatch(/Preview/);
    const missingApp = await fetch(`${origin}/__stillwind/compile`, {
      method: "POST",
      body: JSON.stringify({ source: "export const value = 1;", theme: "" }),
    });
    const failed = (await missingApp.json()) as {
      diagnostics: { code: string }[];
      previewUrl: string | null;
    };
    expect(failed.previewUrl).toBe(null);
    expect(failed.diagnostics[0]?.code).toBe("SHPLAY001");
    expect((await fetch(`${origin}/__stillwind/preview?id=missing`)).status).toBe(404);
  } finally {
    await server.close();
  }
});
