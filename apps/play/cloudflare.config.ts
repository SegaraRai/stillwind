import { defineConfig } from "cf/config";

export default defineConfig(({ isPreview }) => ({
  worker: {
    name: "stillwind-play",
    compatibilityDate: "2026-10-29",
    workersDev: false,
    previewUrls: true,
    domains: isPreview ? [] : ["play.stillwind.dev"],
    assets: {
      notFoundHandling: "404-page",
    },
  },
}));
