import { iconify, svgDirectory } from "@stillwind/plugin-iconify";
import { defineConfig, lintPresets } from "stillwind/config";

export default defineConfig({
  projects: [
    {
      css: "src/styles.css",
      files: ["src/**/*.{ts,astro}"],
      lint: {
        extends: [lintPresets.strict, lintPresets.stylistic],
        rules: {
          "unknown-utility": "deny",
          "invalid-utility": "deny",
          "no-arbitrary-breakpoints": "deny",
          "no-arbitrary-colors": "deny",
        },
      },
      plugins: [
        iconify({
          collections: {
            stillwind: svgDirectory("src/icons/stillwind"),
          },
        }),
      ],
    },
  ],
});
