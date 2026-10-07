import { resolveFixtureCss } from "@stillwind-internal/tools-fixture-css/stillwind-css";
import { defineConfig } from "vite-plus";
import { playwright } from "vite-plus/test/browser-playwright";

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
    include: ["compiler/canonical.browser.test.ts"],
    testTimeout: 120_000,
    provide: {
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
