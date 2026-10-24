import { defineFixture } from "@stillwind-internal/e2e-harness";

export default defineFixture({
  id: "vite8-lit3",
  support: {
    tier: "primary",
  },
  driver: "vite",
  versions: ["vite-plus", "lit"],
  profiles: ["default"],
  page: {
    path: "/",
    ready: '[data-testid="styled"]',
    target: '[data-testid="styled"]',
    css: {
      color: "rgb(18, 52, 86)",
      paddingTop: "12px",
      marginTop: "11px",
    },
    styles: [
      {
        selector: '[data-testid="dynamic"]',
        css: { color: "rgb(255, 0, 0)" },
      },
    ],
    updates: [
      {
        trigger: '[data-testid="dynamic-toggle"]',
        target: '[data-testid="dynamic"]',
        css: { color: "rgb(0, 0, 255)" },
      },
    ],
    counters: [
      {
        selector: '[data-testid="counter"]',
        initial: "0",
        clicked: "1",
      },
    ],
  },
  ssr: {
    status: "not-applicable",
    reason: "The Lit element directive supports browser rendering only.",
  },
  hydration: {
    status: "not-applicable",
    reason: "The Lit fixture renders into a shadow root in the browser.",
  },
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
      file: "src/App.ts",
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
  semantics: [
    {
      id: "main",
      interactive: true,
    },
  ],
});
