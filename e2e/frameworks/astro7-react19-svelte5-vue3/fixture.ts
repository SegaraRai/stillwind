import { defineFixture } from "@stillwind/e2e-harness";

export default defineFixture({
  id: "astro7-react19-svelte5-vue3",
  semantics: [
    { id: "react", interactive: true },
    { id: "svelte", interactive: true },
    { id: "vue", interactive: true },
  ],
  driver: "astro",
  versions: ["vite-plus", "astro", "react", "react-dom", "svelte", "vue"],
  profiles: ["default", "split"],
  page: {
    path: "/",
    ready: "html[data-react-ready][data-svelte-ready][data-vue-ready]",
    target: '[data-testid="styled"]',
    css: {
      color: "rgb(18, 52, 86)",
      paddingTop: "12px",
      marginTop: "11px",
    },
    styles: [
      { selector: '[data-testid="react-counter"]', css: { color: "rgb(18, 52, 86)" } },
      { selector: '[data-testid="svelte-counter"]', css: { color: "rgb(18, 52, 86)" } },
      { selector: '[data-testid="vue-counter"]', css: { color: "rgb(18, 52, 86)" } },
    ],
    counters: [
      {
        selector: '[data-testid="react-counter"]',
        initial: "0",
        clicked: "1",
      },
      {
        selector: '[data-testid="svelte-counter"]',
        initial: "0",
        clicked: "1",
      },
      {
        selector: '[data-testid="vue-counter"]',
        initial: "0",
        clicked: "1",
      },
    ],
  },
  ssr: {
    status: "not-applicable",
    reason: "Astro prerenders this static application.",
  },
  hydration: true,
  prerender: true,
  edits: {
    theme: {
      file: "src/styles.css",
      from: "--color-probe: #123456",
      to: "--color-probe: #654321",
      css: {
        color: "rgb(101, 67, 33)",
      },
      document: "reload",
      state: "reset",
    },
    component: {
      file: "src/pages/index.astro",
      from: "p-[12px]",
      to: "p-[20px]",
      css: {
        paddingTop: "20px",
      },
      document: "reload",
      state: "reset",
    },
    shared: {
      file: "src/shared-style.ts",
      from: 'style("shared")',
      to: 'style("m-[23px]")',
      css: {
        marginTop: "23px",
      },
      document: "reload",
      state: "reset",
    },
  },
});
