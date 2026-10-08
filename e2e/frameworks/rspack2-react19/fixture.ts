import { defineFixture } from "@stillwind-internal/e2e-harness";

export default defineFixture({
  id: "rspack2-react19",
  support: {
    tier: "compatibility",
  },
  semantics: [{ id: "main", interactive: true }],
  driver: "rspack",
  versions: ["vite-plus", "react", "react-dom"],
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
    reason: "The application has only a client entrypoint.",
  },
  hydration: {
    status: "not-applicable",
    reason: "The application has only a client entrypoint.",
  },
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
      file: "src/App.tsx",
      from: "p-[12px]",
      to: "p-[20px]",
      css: {
        paddingTop: "20px",
      },
      document: "preserve",
      state: "reset",
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
