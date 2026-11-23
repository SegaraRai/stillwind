import { vitePlusFormatHook } from "@stillwind-internal/tools-workspace/format";
import { iconify, svgDirectory } from "@stillwind/plugin-iconify";
import { defineConfig, lintPresets } from "stillwind/config";

export default defineConfig({
  projects: [
    {
      css: "src/styles.css",
      files: ["src/**/*.{astro,ts}"],
      hooks: [vitePlusFormatHook],
      lint: {
        extends: [lintPresets.strict, lintPresets.stylistic],
        rules: {
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
