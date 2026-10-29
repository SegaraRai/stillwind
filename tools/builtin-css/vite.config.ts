import { testCache } from "@stillwind-internal/tools-workspace/task-cache";
import { defineConfig } from "vite-plus";

export default defineConfig({
  root: import.meta.dirname,
  test: {
    name: "builtin-css",
    include: ["*.test.ts"],
  },
  run: {
    tasks: {
      "generate:builtin-css": {
        command: "node generate.ts",
        cache: {
          input: [{ auto: true }, "../../packages/core/index.css", "../../packages/core/css/**"],
          output: ["../../crates/test-support/assets/*.css"],
        },
      },
      "check:generated:builtin-css": {
        command: "node generate.ts --check",
        cache: {
          input: [
            { auto: true },
            "../../packages/core/index.css",
            "../../packages/core/css/**",
            "../../crates/test-support/assets/*.css",
          ],
          output: [],
        },
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
