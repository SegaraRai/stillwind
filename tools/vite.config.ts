import { defineConfig } from "vite-plus";

export default defineConfig({
  test: {
    include: ["tailwind/*.test.ts"],
    exclude: ["tailwind/browser.test.ts"],
    testTimeout: 30_000,
  },
  run: {
    tasks: {
      typecheck: {
        command: "tsc --noEmit",
        dependsOn: ["@stillwind/host#build"],
      },
      test: {
        command: "vp test --run",
        dependsOn: ["@stillwind/native#build"],
        cache: false,
      },
      "webref:generate": {
        command: "node css/webref.ts",
        cache: false,
      },
      "webref:check": {
        command: "node css/webref.ts --check",
        cache: false,
      },
      "projects:refresh": {
        command: "node projects/fixtures.ts",
        dependsOn: ["@stillwind/host#build"],
        cache: false,
      },
      "projects:check": {
        command: "node projects/fixtures.ts --check",
        dependsOn: ["@stillwind/host#build"],
        cache: false,
      },
      "tailwind:fixtures:refresh": {
        command: "node tailwind/fixtures.ts --write",
        cache: false,
      },
      "tailwind:fixtures:check": {
        command: "node tailwind/fixtures.ts",
        cache: false,
      },
      "tailwind:order:generate": {
        command: "node tailwind/utility-order.ts",
        cache: false,
      },
      "tailwind:order:check": {
        command: "node tailwind/utility-order.ts --check",
        cache: false,
      },
      "tailwind:parity": {
        command: "node tailwind/parity.ts",
        dependsOn: ["@stillwind/native#build"],
        cache: false,
      },
      "tailwind:property-order": {
        command: "node tailwind/check-property-order.ts",
        dependsOn: ["@stillwind/native#build"],
        cache: false,
      },
      "tailwind:numeric-domains": {
        command: "node tailwind/numeric-domains.ts",
        dependsOn: ["@stillwind/native#build"],
        cache: false,
      },
      "tailwind:inventory": {
        command: "node tailwind/inventory.ts",
        dependsOn: ["@stillwind/native#build"],
        cache: false,
      },
      "tailwind:analyze": {
        command: "node tailwind/parity.ts --analyze --quiet --summary-only",
        dependsOn: ["@stillwind/native#build"],
        cache: false,
      },
      "tailwind:registrations": {
        command: "node tailwind/registrations.ts",
        cache: false,
      },
      "tailwind:browser": {
        command: "vp test --run --config browser.config.ts",
        dependsOn: ["@stillwind/native#build"],
        cache: false,
      },
    },
  },
});
