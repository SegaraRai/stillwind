import react from "@astrojs/react";
import svelte from "@astrojs/svelte";
import vue from "@astrojs/vue";
import { stillwind } from "@stillwind/integration/astro";
import { defineConfig } from "astro/config";

export default defineConfig({
  integrations: [
    stillwind({
      emission: process.env.STILLWIND_E2E_PROFILE === "split" ? "split" : "single",
    }),
    react(),
    svelte(),
    vue(),
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
