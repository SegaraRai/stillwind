import { defineFixture } from "@stillwind-internal/e2e-harness";

export default defineFixture({
  id: "next16-react19",
  support: { tier: "primary", profiles: { standalone: "compatibility" } },
  semantics: [{ id: "main", interactive: true }],
  driver: "next",
  versions: ["next", "react"],
  profiles: ["turbopack", "webpack", "standalone"],
  page: {
    path: "/",
    ready: 'html[data-hydrated="yes"]',
    target: "#server",
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
      "rsc-flight",
      "server-only-isolation",
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
