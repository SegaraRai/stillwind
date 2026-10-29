import { fixtureCache, testCache } from "@stillwind-internal/tools-workspace/task-cache";
import { defineConfig } from "vite-plus";

export default defineConfig({
  root: import.meta.dirname,
  test: {
    name: "tailwind",
    include: ["*.test.ts"],
    testTimeout: 30000,
  },
  run: {
    tasks: {
      "prepare:tailwind-source": {
        command: "node prepare-upstream.ts",
      },
      "generate:tailwind-banner": {
        command: "node banner.ts",
        cache: {
          input: [{ auto: true }, "../../pnpm-lock.yaml"],
          output: ["../../crates/compiler/src/banner/tailwind.rs"],
        },
      },
      "check:generated:tailwind-banner": {
        command: "node banner.ts --check",
        cache: {
          input: [
            { auto: true },
            "../../pnpm-lock.yaml",
            "../../crates/compiler/src/banner/tailwind.rs",
          ],
          output: [],
        },
      },
      "generate:tailwind-order": {
        command: "node utility-order.ts",
        cache: fixtureCache,
      },
      "check:generated:tailwind-order": {
        command: "node utility-order.ts --check",
        cache: fixtureCache,
      },
      "generate:tailwind-registrations": {
        command: "node registrations.ts",
        cache: fixtureCache,
        dependsOn: ["prepare:tailwind-source"],
      },
      "check:tailwind-registrations": {
        command: "node registrations.ts --check",
        cache: fixtureCache,
        dependsOn: ["prepare:tailwind-source"],
      },
      "generate:fixtures:tailwind": {
        command: "node fixtures.ts",
        cache: fixtureCache,
        dependsOn: ["generate:tailwind-registrations"],
      },
      "check:generated:fixtures:tailwind": {
        command: "node fixtures.ts --check",
        cache: fixtureCache,
        dependsOn: ["check:tailwind-registrations"],
      },
      "generate:fixtures:tailwind-order": {
        command: "node utility-order-fixtures.ts",
        cache: fixtureCache,
      },
      "check:generated:fixtures:tailwind-order": {
        command: "node utility-order-fixtures.ts --check",
        cache: fixtureCache,
      },
      typecheck: {
        command: "tsc --noEmit",
      },
      test: {
        command: "vp test --run",
        cache: testCache,
      },
    },
  },
});
