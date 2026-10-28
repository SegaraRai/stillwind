import { defineConfig, lintPresets } from "stillwind/config";

export default defineConfig({
  projects: [
    {
      css: "stillwind.css",
      files: ["**/*.{ts,tsx}"],
      lint: {
        extends: [lintPresets.recommended],
        rules: {
          "no-arbitrary-colors": "deny",
          "no-arbitrary-spacing": "deny",
          "no-arbitrary-breakpoints": "deny",
        },
      },
    },
  ],
});
