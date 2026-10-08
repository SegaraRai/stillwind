import { defineConfig } from "vite-plus";

export default defineConfig({
  run: {
    tasks: {
      "generate:lint-config": {
        command: "node lint-rules.ts",
        cache: {
          input: [
            { auto: true },
            "../../crates/diagnostics/src/lint/config.rs",
            "../../crates/css/src/values/policy.rs",
          ],
          output: [
            "../../crates/napi/src/options/lint.rs",
            "../../packages/config/src/lint-rules.ts",
          ],
        },
      },
      "check:generated:lint-config": {
        command: "node lint-rules.ts --check",
        cache: {
          input: [
            { auto: true },
            "../../crates/diagnostics/src/lint/config.rs",
            "../../crates/css/src/values/policy.rs",
            "../../crates/napi/src/options/lint.rs",
            "../../packages/config/src/lint-rules.ts",
          ],
        },
      },
      typecheck: {
        command: "tsc --noEmit",
      },
    },
  },
});
