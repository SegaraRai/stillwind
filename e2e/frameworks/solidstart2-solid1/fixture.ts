import { defineFixture } from "@stillwind/e2e-harness";

export default defineFixture({
  id: "solidstart2-solid1",
  semantics: [{ id: "main", interactive: true }],
  driver: "vite",
  versions: ["@solidjs/start", "solid-js"],
  profiles: ["default"],
  page: {
    path: "/",
    ready: 'html[data-hydrated="yes"]',
    target: "#static",
    css: {
      color: "rgb(11, 22, 33)",
    },
    counters: [
      {
        selector: "#client",
        initial: "-1",
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
