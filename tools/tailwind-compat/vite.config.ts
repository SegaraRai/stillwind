import { testCache } from "@stillwind-tools/workspace/task-cache";
import { defineConfig } from "vite-plus";

const parityCache = { ...testCache, env: ["STILLWIND_PARITY_*", "NAPI_RS_*"] };

export default defineConfig({
  root: import.meta.dirname,
  test: {
    name: "tailwind-compat",
    include: ["*.test.ts"],
    exclude: ["browser.test.ts"],
    testTimeout: 30000,
  },
  run: {
    tasks: {
      "analyze:tailwind": {
        command: "node parity.ts --analyze --quiet --summary-only",
        cache: parityCache,
        dependsOn: ["@stillwind/native#build"],
      },
      "analyze:tailwind:inventory": {
        command: "node inventory.ts",
        cache: false,
        dependsOn: ["@stillwind/native#build"],
      },
      typecheck: {
        command: "tsc --noEmit",
        dependsOn: ["@stillwind/native#build"],
      },
      test: {
        command: "vp test --run",
        cache: testCache,
        dependsOn: ["@stillwind/native#build"],
      },
      "test:browser:tailwind": {
        command: "vp test --run --config vite.browser.config.ts",
        cache: false,
        dependsOn: ["@stillwind/native#build"],
      },
      "test:tailwind:parity": {
        command: "node parity.ts",
        cache: parityCache,
        dependsOn: ["@stillwind/native#build"],
      },
      "test:tailwind:property-order": {
        command: "node check-property-order.ts",
        cache: parityCache,
        dependsOn: ["@stillwind/native#build"],
      },
      "test:tailwind:numeric-domains": {
        command: "node numeric-domains.ts",
        cache: parityCache,
        dependsOn: ["@stillwind/native#build"],
      },
    },
  },
});
