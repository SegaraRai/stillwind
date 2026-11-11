import { readFile } from "node:fs/promises";

import react from "@astrojs/react";
import { siteHtml } from "@stillwind-internal/tools-site-html/astro";
import { stillwind } from "@stillwind/integration/astro";
import { defineConfig } from "astro/config";
import { version } from "stillwind/package.json" with { type: "json" };

import { previewScript } from "./src/preview/channel";

// The threaded WASM compiler needs a cross-origin isolated page.
const headers = {
  "Cross-Origin-Opener-Policy": "same-origin",
  "Cross-Origin-Embedder-Policy": "require-corp",
};

export default defineConfig({
  site: process.env.PUBLIC_PLAYGROUND_URL ?? "https://play.stillwind.dev",
  trailingSlash: "never",
  // `build/assets.ts` generates the static files, including those copied from `public/`.
  publicDir: "dist/assets-static",
  outDir: "dist/site",
  server: {
    port: 5177,
    headers,
  },
  integrations: [
    stillwind(),
    react(),
    siteHtml({
      cloudflareWebAnalytics: true,
      contentSecurityPolicy: {
        "default-src": ["'self'"],
        // The extension host and the preview frame run code with `new Function`. The language
        // server imports compiled configuration files and starts workers from blob URLs.
        "script-src": ["'self'", "'unsafe-eval'", "'wasm-unsafe-eval'", "blob:"],
        // The editor inserts style elements.
        "style-src": ["'self'", "'unsafe-inline'"],
        // The editor reads bundled extension files from data URLs.
        "connect-src": ["'self'", "data:", "blob:"],
        "img-src": ["'self'", "data:"],
        "object-src": ["'none'"],
        "base-uri": ["'none'"],
        "form-action": ["'self'"],
        // The extension host runs in a same-origin frame.
        "frame-ancestors": ["'self'"],
      },
      // The preview frame inherits the policy and embeds the runtime as an inline script.
      runtimeInlineScripts: async (output) => [
        previewScript(await readFile(new URL("preview-runtime.js", output), "utf8")),
      ],
    }),
  ],
  vite: {
    define: {
      __STILLWIND_VERSION__: JSON.stringify(version),
    },
    resolve: {
      alias: {
        "monaco-editor": "@codingame/monaco-vscode-editor-api",
        vscode: "@codingame/monaco-vscode-extension-api",
      },
      dedupe: ["@codingame/monaco-vscode-api"],
    },
    build: {
      target: "esnext",
    },
    worker: {
      format: "es",
    },
    optimizeDeps: {
      exclude: [
        "@codingame/monaco-vscode-api",
        "@codingame/monaco-vscode-editor-api",
        "@codingame/monaco-vscode-extension-api",
        // Its WASM binding and thread worker load from URLs relative to the package files.
        "@rolldown/browser",
      ],
    },
  },
});
