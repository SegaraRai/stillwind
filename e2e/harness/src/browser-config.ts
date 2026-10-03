import { defineConfig } from "vite-plus";
import { playwright } from "vite-plus/test/browser-playwright";

export default function browserConfig() {
  return defineConfig({
    server: {
      headers: {
        "Cross-Origin-Opener-Policy": "same-origin",
        "Cross-Origin-Embedder-Policy": "require-corp",
      },
    },
    test: {
      browser: {
        enabled: true,
        headless: true,
        provider: playwright({
          launchOptions: process.env.STILLWIND_BROWSER
            ? { executablePath: process.env.STILLWIND_BROWSER }
            : { channel: "chrome" },
        }),
        instances: [{ browser: "chromium" }],
      },
    },
  });
}
