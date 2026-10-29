import { defineConfig } from "cf/config";

export default defineConfig(({ isPreview }) => ({
  worker: {
    name: "stillwind-website",
    compatibilityDate: "2026-10-29",
    workersDev: false,
    previewUrls: true,
    domains: isPreview ? [] : ["stillwind.dev"],
    assets: {
      notFoundHandling: "404-page",
    },
  },
}));
