import { defineFixture } from "@stillwind-internal/e2e-harness";

export default defineFixture({
  id: "nuxt4-vue3",
  support: {
    tier: "primary",
  },
  semantics: [{ id: "main", interactive: true }],
  driver: "nuxt",
  versions: ["vite-plus", "nuxt", "vue"],
  profiles: ["default", "split"],
  page: {
    path: "/",
    ready: 'html[data-hydrated="yes"]',
    target: '[data-testid="styled"]',
    css: {
      color: "rgb(18, 52, 86)",
      paddingTop: "12px",
      marginTop: "11px",
    },
    counters: [
      {
        selector: '[data-testid="counter"]',
        initial: "0",
        clicked: "1",
      },
    ],
  },
  ssr: true,
  hydration: true,
  edits: {
    theme: {
      file: "src/styles.css",
      from: "--color-probe: #123456",
      to: "--color-probe: #654321",
      css: {
        color: "rgb(101, 67, 33)",
      },
      document: "preserve",
      state: "preserve",
    },
    component: {
      file: "app/app.vue",
      from: "p-[12px]",
      to: "p-[20px]",
      css: {
        paddingTop: "20px",
      },
      document: "preserve",
      state: "preserve",
    },
    shared: {
      file: "src/shared-style.ts",
      from: 'style("shared")',
      to: 'style("m-[23px]")',
      css: {
        marginTop: "23px",
      },
      document: "preserve",
      state: "reset",
    },
  },
});
