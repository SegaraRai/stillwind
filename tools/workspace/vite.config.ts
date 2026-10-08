import { defineConfig } from "vite-plus";

export default defineConfig({
  run: {
    tasks: {
      "generate:lint-config": {
        command: "node lint-rules.ts",
        cache: {
          input: [
            { auto: true },
            "../../crates/css/src/values/policy.rs",
            "../../crates/diagnostics/src/lint/config/catalog.rs",
            "../../crates/diagnostics/src/lint/config/policies.rs",
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
            "../../crates/css/src/values/policy.rs",
            "../../crates/diagnostics/src/lint/config/catalog.rs",
            "../../crates/diagnostics/src/lint/config/policies.rs",
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
