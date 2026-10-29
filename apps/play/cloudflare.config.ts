import { defineConfig } from "cf/config";

export default defineConfig(({ isPreview }) => ({
  worker: {
    name: "stillwind-play",
    compatibilityDate: "2026-10-29",
    workersDev: false,
    previewUrls: true,
    domains: [
      ...(isPreview ? [] : ["play.stillwind.dev"]),
      { hostname: "play-preview.stillwind.dev", production: false, previews: true },
    ],
    assets: {
      notFoundHandling: "404-page",
    },
  },
}));
