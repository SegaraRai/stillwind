import { defineFixture } from "@stillwind-internal/e2e-harness";

export default defineFixture({
  id: "vite8-react19-reactrouter8",
  support: {
    tier: "primary",
  },
  semantics: [{ id: "main", interactive: true }],
  driver: "react-router",
  versions: ["@react-router/dev", "react"],
  profiles: ["default"],
  page: {
    path: "/",
    ready: 'html[data-hydrated="yes"]',
    target: "#client",
    css: {
      color: "rgb(11, 22, 33)",
    },
    counters: [
      {
        selector: "#client",
        initial: "0",
        clicked: "1",
      },
    ],
  },
  ssr: true,
  hydration: true,
  edits: {
    theme: {
      file: "root.css",
      from: "rgb(11, 22, 33)",
      to: "rgb(44,55,66)",
      css: {
        color: "rgb(44, 55, 66)",
      },
      document: "preserve",
      state: "preserve",
    },
  },
  production: false,
  specialized: {
    contracts: [
      "rendering",
      "production",
      "cold-build",
      "warm-build",
      "ssr",
      "hydration",
      "navigation",
      "css-edit",
      "macro-removal",
      "source-map",
    ],
  },
});
