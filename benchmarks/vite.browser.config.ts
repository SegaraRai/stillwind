import { readFileSync, readdirSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join, resolve } from "node:path";

import { resolveFixtureCss } from "@stillwind-internal/tools-fixture-css/stillwind-css";
import { defineConfig } from "vite-plus";
import { playwright } from "vite-plus/test/browser-playwright";

const core = resolve(import.meta.dirname, "../packages/core");
const library = join(
  dirname(
    createRequire(resolve(import.meta.dirname, "../packages/host/package.json")).resolve(
      "typescript-6/package.json",
    ),
  ),
  "lib",
);
const declarations: [string, string][] = [
  [
    "/workspace/node_modules/stillwind/package.json",
    readFileSync(join(core, "package.json"), "utf8"),
  ],
  ...readDeclarations(join(core, "dist"), "/workspace/node_modules/stillwind/dist"),
  ...readDeclarations(library, "/node_modules/typescript/lib"),
];

/**
 * Copies declaration files into the virtual paths used by browser benchmarks.
 *
 * @param directory - Physical directory to read recursively.
 * @param virtual - Absolute virtual directory, using forward slashes.
 * @returns Virtual filenames paired with their UTF-8 declaration text. Other files are skipped.
 * @throws If a directory or declaration file cannot be read.
 */
function readDeclarations(directory: string, virtual: string): [string, string][] {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const file = join(directory, entry.name);
    const id = `${virtual}/${entry.name}`;
    if (entry.isDirectory()) {
      return readDeclarations(file, id);
    }
    return entry.name.endsWith(".d.ts") ? [[id, readFileSync(file, "utf8")]] : [];
  });
}

export default defineConfig({
  root: import.meta.dirname,
  server: {
    headers: {
      "Cross-Origin-Opener-Policy": "same-origin",
      "Cross-Origin-Embedder-Policy": "require-corp",
    },
  },
  test: {
    name: "canonical-browser",
    include: ["compiler/canonical.browser.test.ts", "compiler/analysis.browser.test.ts"],
    testTimeout: 120_000,
    provide: {
      analysisBrowserFiles: declarations,
      canonicalBrowserCss: resolveFixtureCss(
        '@import "stillwind"; @theme { --spacing:.25rem; --color-brand:red; }',
      ),
    },
    browser: {
      enabled: true,
      headless: true,
      provider: playwright({
        launchOptions: process.env.STILLWIND_BROWSER
          ? {
              executablePath: process.env.STILLWIND_BROWSER,
            }
          : {
              channel: "chrome",
            },
      }),
      instances: [{ browser: "chromium" }],
    },
  },
});
