import { defineConfig } from "cf/config";

export default defineConfig(({ isPreview }) => ({
  worker: {
    name: "stillwind-website",
    compatibilityDate: "2026-10-29",
    workersDev: false,
    previewUrls: true,
    domains: [
      ...(isPreview ? [] : ["stillwind.dev"]),
      {
        hostname: "preview.stillwind.dev",
        production: false,
        previews: true,
      },
    ],
    assets: {
      htmlHandling: "drop-trailing-slash",
      notFoundHandling: "404-page",
    },
  },
}));
