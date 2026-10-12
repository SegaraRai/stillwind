import { stillwind } from "@stillwind/integration/astro";
import { defineConfig } from "astro/config";

export default defineConfig({
  compressHTML: "jsx",
  integrations: [stillwind()],
  vite: {
    build: {
      rolldownOptions: { external: ["@stillwind/host", "@stillwind/native"] },
    },
  },
  server: {
    port: 4322,
  },
  build: {
    inlineStylesheets: "never",
  },
});
