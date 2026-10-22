import nimbus, { defineConfig as defineNimbusConfig } from "@cloudflare/nimbus-docs";
import { stillwind } from "@stillwind/integration/astro";
import { defineConfig } from "astro/config";

import { referenceMarkdown } from "./src/components/docs/reference/markdown";

const { componentMap, sourcePlugin } = await referenceMarkdown();

export default defineConfig({
  compressHTML: "jsx",
  redirects: {
    "/docs/configuration/lint/": "/docs/configuration/#lint",
  },
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
        sidebar: {
          items: [
            {
              label: "Introduction",
              items: ["docs/why", "docs", "docs/guides/migrating-from-tailwind"],
            },
            {
              label: "Writing styles",
              items: [
                "docs/style",
                "docs/guides/reusable-styles",
                "docs/guides/responsive-styles",
                "docs/guides/animations",
                "docs/guides/view-transitions",
              ],
            },
            {
              label: "Keeping styles in order",
              items: [
                "docs/guides/design-systems",
                "docs/guides/element-relationships",
                "docs/guides/lint-adoption",
              ],
            },
            {
              label: "Integrations",
              items: ["docs/frameworks", "docs/how-styles-compile", "docs/guides/shadow-dom"],
            },
            {
              label: "Reference",
              items: [
                "docs/configuration",
                {
                  label: "Macros",
                  autogenerate: { directory: "docs/macros" },
                },
                "docs/utilities",
                {
                  label: "Lint rules",
                  autogenerate: { directory: "docs/lints" },
                },
                {
                  label: "Compiler errors",
                  autogenerate: { directory: "docs/errors" },
                },
              ],
            },
          ],
        },
      }),
      {
        markdown: {
          componentMap,
          mdastPlugins: [sourcePlugin],
        },
        rules: {
          "nimbus/frontmatter-shape": "error",
          "nimbus/internal-link": "error",
        },
      },
    ),
    stillwind(),
  ],
  vite: {
    ssr: {
      external: ["@stillwind/host", "@stillwind/native"],
    },
    build: {
      rolldownOptions: {
        external: ["@stillwind/host", "@stillwind/native"],
      },
    },
  },
  server: {
    port: 4322,
  },
  build: {
    inlineStylesheets: "never",
  },
});
