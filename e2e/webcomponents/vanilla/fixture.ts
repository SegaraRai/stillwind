import { defineFixture } from "@stillwind-internal/e2e-harness";

export default defineFixture({
  id: "webcomponents-vanilla",
  support: {
    tier: "primary",
  },
  driver: "vite",
  versions: ["vite-plus"],
  profiles: ["default"],
  page: {
    path: "/",
    ready: '[data-testid="first"] [data-testid="styled"]',
    target: '[data-testid="first"] [data-testid="styled"]',
    css: {
      color: "rgb(18, 52, 86)",
      paddingTop: "12px",
      marginTop: "11px",
    },
    styles: [
      {
        selector: '[data-testid="first"] [data-testid="dynamic"]',
        css: {
          color: "rgb(255, 0, 0)",
        },
      },
      {
        selector: '[data-testid="second"] [data-testid="dynamic"]',
        css: {
          color: "rgb(0, 128, 0)",
        },
      },
      {
        selector: '[data-testid="first"] [data-testid="authored"]',
        css: {
          borderTopWidth: "3px",
        },
      },
    ],
    updates: [
      {
        trigger: '[data-testid="first"] [data-testid="dynamic-toggle"]',
        target: '[data-testid="first"] [data-testid="dynamic"]',
        css: {
          color: "rgb(0, 0, 255)",
        },
      },
    ],
    counters: [
      {
        selector: '[data-testid="first"] [data-testid="counter"]',
        initial: "0",
        clicked: "1",
      },
      {
        selector: '[data-testid="second"] [data-testid="counter"]',
        initial: "0",
        clicked: "1",
      },
    ],
  },
  ssr: {
    status: "not-applicable",
    reason: "The example defines browser custom elements.",
  },
  hydration: {
    status: "not-applicable",
    reason: "The custom elements render in the browser.",
  },
  shadowDom: {
    hosts: ['[data-testid="first"]', '[data-testid="second"]'],
    target: '[data-testid="styled"]',
    outside: '[data-testid="outside"]',
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
      file: "src/Probe.ts",
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
