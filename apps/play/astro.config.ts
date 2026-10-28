import react from "@astrojs/react";
import { stillwind } from "@stillwind/integration/astro";
import { defineConfig } from "astro/config";
import { version } from "stillwind/package.json" with { type: "json" };

// The threaded WASM compiler needs a cross-origin isolated page.
const headers = {
  "Cross-Origin-Opener-Policy": "same-origin",
  "Cross-Origin-Embedder-Policy": "require-corp",
};

export default defineConfig({
  site: "https://play.stillwind.dev",
  // `build/assets.ts` generates the static files, including those copied from `public/`.
  publicDir: "dist/assets-static",
  outDir: "dist/site",
  server: {
    port: 5177,
    headers,
  },
  integrations: [stillwind(), react()],
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
      sourcemap: true,
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
