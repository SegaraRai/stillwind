import { defineFixture } from "@stillwind-internal/e2e-harness";

export default defineFixture({
  id: "astro7-plain",
  support: { tier: "primary" },
  semantics: [{ id: "main", interactive: false }],
  driver: "astro",
  versions: ["vite-plus", "astro"],
  profiles: ["default", "split"],
  page: {
    path: "/",
    ready: '[data-testid="styled"]',
    target: '[data-testid="styled"]',
    css: {
      color: "rgb(18, 52, 86)",
      paddingTop: "12px",
      marginTop: "11px",
    },
    counters: [],
  },
  ssr: {
    status: "not-applicable",
    reason: "Astro prerenders this static application.",
  },
  hydration: {
    status: "not-applicable",
    reason: "The page has no hydrated components.",
  },
  prerender: true,
  edits: {
    theme: {
      file: "src/styles.css",
      from: "--color-probe: #123456",
      to: "--color-probe: #654321",
      css: {
        color: "rgb(101, 67, 33)",
      },
      document: "preserve",
      state: "not-applicable",
    },
    component: {
      file: "src/pages/index.astro",
      from: "p-[12px]",
      to: "p-[20px]",
      css: {
        paddingTop: "20px",
      },
      document: "reload",
      state: "not-applicable",
    },
    shared: {
      file: "src/shared-style.ts",
      from: 'style("shared")',
      to: 'style("m-[23px]")',
      css: {
        marginTop: "23px",
      },
      document: "reload",
      state: "not-applicable",
    },
  },
});
