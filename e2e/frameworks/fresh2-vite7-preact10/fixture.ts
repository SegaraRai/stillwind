import { defineFixture } from "@stillwind-internal/e2e-harness";

export default defineFixture({
  id: "fresh2-vite7-preact10",
  support: {
    tier: "compatibility",
  },
  semantics: [{ id: "main", interactive: true }],
  driver: "fresh",
  versions: ["preact"],
  platformFailure: {
    platforms: ["linux", "darwin"],
    reason: "Deno pipe handles do not expose descriptors for the TypeScript checker",
  },
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
  ssr: true,
  hydration: true,
  navigation: {
    link: '[data-testid="next"]',
    target: '[data-testid="route-styled"]',
    css: { paddingTop: "17px", color: "rgb(171, 205, 239)" },
    back: '[data-testid="back"]',
    document: "reload",
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
      file: "src/App.tsx",
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
