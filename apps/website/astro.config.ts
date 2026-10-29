import nimbus, { defineConfig as defineNimbusConfig } from "@cloudflare/nimbus-docs";
import { stillwind } from "@stillwind/integration/astro";
import { defineConfig } from "astro/config";

import { referenceMarkdown } from "./src/components/docs/reference/markdown";

const { componentMap, sourcePlugin } = await referenceMarkdown();

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
        socialImage: "/og.png",
        search: false,
        sidebar: {
          items: [
            "docs", // Getting Started
            {
              label: "Introduction",
              items: ["docs/why", "docs/guides/migrating-from-tailwind"],
            },
            {
              label: "Writing styles",
              items: [
                "docs/guides/style",
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
              label: "Concepts",
              items: ["docs/concepts/how-styles-compile", "docs/concepts/css-ordering"],
            },
            {
              label: "Integrations",
              items: ["docs/integrations", "docs/guides/web-components"],
            },
            {
              label: "Reference",
              items: [
                "docs/utilities",
                "docs/configuration",
                {
                  label: "Macros",
                  items: [
                    "docs/macros",
                    "docs/macros/style",
                    "docs/macros/anchor-scope",
                    "docs/macros/css",
                    "docs/macros/cva",
                    "docs/macros/keyframes",
                    "docs/macros/media",
                    "docs/macros/view-transition",
                    "docs/macros/ref",
                    "docs/macros/ref/condition",
                    "docs/macros/ref/in",
                    "docs/macros/ref/at",
                    "docs/macros/ref/as-container",
                    "docs/macros/ref/as-anchor",
                    "docs/macros/ref/position-anchor",
                  ],
                },
                {
                  label: "At-rules",
                  autogenerate: {
                    directory: "docs/at-rules",
                  },
                },
                {
                  label: "Lint rules",
                  autogenerate: {
                    directory: "docs/lints",
                  },
                },
                {
                  label: "Compiler errors",
                  autogenerate: {
                    directory: "docs/errors",
                  },
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
    worker: {
      format: "es",
    },
  },
  server: {
    port: 4322,
    // The utility lookup runs the threaded WASM compiler, which needs a cross-origin isolated
    // page. Deployments set these headers for that page only in public/_headers.
    headers: {
      "Cross-Origin-Opener-Policy": "same-origin",
      "Cross-Origin-Embedder-Policy": "require-corp",
    },
  },
  build: {
    inlineStylesheets: "never",
  },
});
