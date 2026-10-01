import { defineFixture } from "@stillwind/e2e-harness";

export default defineFixture({
  id: "tanstackstart1-react19",
  semantics: [{ id: "main", interactive: true }],
  driver: "vite",
  versions: ["vite-plus", "react", "react-dom", "@tanstack/react-start", "@tanstack/react-router"],
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
      file: "src/routes/index.tsx",
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
      state: "preserve",
    },
  },
});
