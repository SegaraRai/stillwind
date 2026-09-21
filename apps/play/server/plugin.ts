import { randomUUID } from "node:crypto";

import type { Plugin } from "vite-plus";

import type { PlayOutput } from "../src/compilation.ts";
import { compilePlayground } from "./compile.ts";
import { readJson, sendJson } from "./http.ts";
import { bundlePreview, previewHtml } from "./preview.ts";

export function playground(css: string): Plugin {
  return {
    name: "stillwind-playground",
    apply: "serve",
    configureServer(server) {
      const previewBundles = new Map<string, string>();
      server.middlewares.use("/__stillwind/compile", (request, response) => {
        if (request.method !== "POST") {
          return sendJson(response, { error: "POST is required" }, 405);
        }
        void readJson(request)
          .then(async ({ source, theme }) => {
            const output = await compilePlayground(source, theme, css, server);
            let previewUrl: string | null = null;
            if (output.diagnostics.length === 0 && output.code) {
              try {
                const id = randomUUID();
                previewBundles.set(
                  id,
                  await bundlePreview(output.code, output.css, server.config.root),
                );
                while (previewBundles.size > 12) {
                  previewBundles.delete(previewBundles.keys().next().value!);
                }
                previewUrl = `/__stillwind/preview?id=${encodeURIComponent(id)}`;
              } catch (error) {
                output.diagnostics.push({
                  code: "SHPLAY001",
                  message: error instanceof Error ? error.message : String(error),
                  file: "Play.tsx",
                  start: 0,
                  end: 0,
                  start_utf16: 0,
                  end_utf16: 0,
                  start_line: 0,
                  start_column: 0,
                  end_line: 0,
                  end_column: 0,
                  notes: [],
                });
              }
            }
            sendJson(response, { ...output, previewUrl } satisfies PlayOutput);
          })
          .catch((error: unknown) =>
            sendJson(
              response,
              { error: error instanceof Error ? error.message : String(error) },
              400,
            ),
          );
      });
      server.middlewares.use("/__stillwind/preview", (request, response) => {
        const id = new URL(request.url ?? "", "http://stillwind.local").searchParams.get("id");
        if (!id || !previewBundles.has(id)) {
          response.statusCode = 404;
          return response.end("Preview expired");
        }
        response.setHeader("content-type", "text/html; charset=utf-8");
        response.setHeader("cache-control", "no-store");
        response.end(previewHtml(id));
      });
      server.middlewares.use("/__stillwind/preview-script", (request, response) => {
        const id = new URL(request.url ?? "", "http://stillwind.local").searchParams.get("id");
        const bundle = id ? previewBundles.get(id) : undefined;
        if (!bundle) {
          response.statusCode = 404;
          return response.end("Preview expired");
        }
        response.setHeader("content-type", "text/javascript; charset=utf-8");
        response.setHeader("cache-control", "no-store");
        response.end(bundle);
      });
    },
  };
}
