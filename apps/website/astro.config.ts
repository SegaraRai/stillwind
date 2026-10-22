import nimbus, { defineConfig as defineNimbusConfig } from "@cloudflare/nimbus-docs";
import { stillwind } from "@stillwind/integration/astro";
import { defineConfig } from "astro/config";

export default defineConfig({
  compressHTML: "jsx",
  integrations: [
    nimbus(
      defineNimbusConfig({
        site: process.env.SITE_URL ?? "http://localhost:4322",
        title: "Stillwind",
        description: "Compile component styles, enforce your design system, and ship static CSS.",
        locale: "en",
        github: "https://github.com/SegaraRai/stillwind",
        editPattern: "https://github.com/SegaraRai/stillwind/edit/main/apps/website/{path}",
        socialImage: "/favicon.svg",
        search: false,
        sidebar: { items: [{ autogenerate: { directory: "docs" } }] },
      }),
      {
        rules: {
          "nimbus/frontmatter-shape": "error",
          "nimbus/internal-link": "error",
        },
      },
    ),
    stillwind(),
  ],
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
