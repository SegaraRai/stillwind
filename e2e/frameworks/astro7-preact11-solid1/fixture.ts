import { defineFixture } from "@stillwind-internal/e2e-harness";

export default defineFixture({
  id: "astro7-preact11-solid1",
  support: { tier: "primary" },
  semantics: [
    { id: "preact", interactive: true },
    { id: "solid", interactive: true },
  ],
  driver: "astro",
  versions: ["vite-plus", "astro", "preact", "solid-js"],
  profiles: ["default", "split"],
  page: {
    path: "/",
    ready: "html[data-preact-ready][data-solid-ready]",
    target: '[data-testid="styled"]',
    css: {
      color: "rgb(18, 52, 86)",
      paddingTop: "12px",
      marginTop: "11px",
    },
    styles: [
      {
        selector: '[data-testid="preact-counter"]',
        css: { color: "rgb(18, 52, 86)" },
      },
      {
        selector: '[data-testid="solid-counter"]',
        css: { color: "rgb(18, 52, 86)" },
      },
    ],
    counters: [
      {
        selector: '[data-testid="preact-counter"]',
        initial: "0",
        clicked: "1",
      },
      {
        selector: '[data-testid="solid-counter"]',
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
    preactIsland: {
      file: "src/components/preact/Counter.tsx",
      from: 'style("text-probe")',
      to: 'style("text-[#d23456]")',
      target: '[data-testid="preact-counter"]',
      css: { color: "rgb(210, 52, 86)" },
      document: "preserve",
      state: "preserve",
      counters: ['[data-testid="preact-counter"]'],
    },
    solidIsland: {
      file: "src/components/solid/Counter.tsx",
      from: 'style("text-probe")',
      to: 'style("text-[#d23456]")',
      target: '[data-testid="solid-counter"]',
      css: { color: "rgb(210, 52, 86)" },
      document: "preserve",
      state: "reset",
      counters: ['[data-testid="solid-counter"]'],
    },
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
