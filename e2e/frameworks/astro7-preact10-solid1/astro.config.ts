import preact from "@astrojs/preact";
import solid from "@astrojs/solid-js";
import { stillwind } from "@stillwind/integration/astro";
import { defineConfig } from "astro/config";

export default defineConfig({
  integrations: [
    stillwind({
      emission: process.env.STILLWIND_E2E_PROFILE === "split" ? "split" : "single",
    }),
    preact({
      include: ["**/preact/**"],
    }),
    solid({
      include: ["**/solid/**"],
    }),
  ],
  build: {
    inlineStylesheets: "never",
  },
  vite: {
    environments: {
      client: {
        build: {
          rolldownOptions: {
            output: {
              chunkFileNames: "_astro/[hash].js",
            },
          },
        },
      },
    },
  },
});
