import nimbus, { defineConfig as defineNimbusConfig } from "@cloudflare/nimbus-docs";
import { stillwind } from "@stillwind/integration/astro";
import { defineConfig } from "astro/config";

import { compiledSourcePlugin } from "./src/components/reference/compiled-source";
import { referenceMarkdown } from "./src/components/reference/markdown";

const componentMap = await referenceMarkdown();

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
                {
                  label: "Configuration",
                  autogenerate: { directory: "docs/configuration" },
                },
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
          mdastPlugins: [compiledSourcePlugin],
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
