import { defineConfig } from "vite-plus";

import { testCache } from "./task-cache.ts";

export default defineConfig({
  test: {
    name: "workspace",
    include: ["*.test.ts"],
  },
  run: {
    tasks: {
      test: {
        command: "vp test --run",
        cache: testCache,
      },
      "generate:lint-config": {
        command: "node lint-rules.ts",
        cache: {
          input: [
            { auto: true },
            "../../crates/css/src/values/policy.rs",
            "../../crates/diagnostics/src/lint/config/catalog.rs",
            "../../crates/diagnostics/src/lint/config/policies.rs",
            "../../crates/diagnostics/src/lint/config/presets.rs",
          ],
          output: [
            "../../crates/napi/src/options/lint.rs",
            "../../packages/config/src/lint-presets.ts",
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
            "../../crates/diagnostics/src/lint/config/presets.rs",
            "../../crates/napi/src/options/lint.rs",
            "../../packages/config/src/lint-presets.ts",
            "../../packages/config/src/lint-rules.ts",
          ],
        },
      },
      "generate:license-copies": {
        command: "node license-copies.ts",
        cache: {
          input: [
            { auto: true },
            "../../LICENSE",
            "../../TAILWINDCSS-MIT-LICENSE",
            "../../WEBREF-MIT-LICENSE",
          ],
          output: [
            "../../packages/vscode-extension/LICENSE",
            "../../packages/*/TAILWINDCSS-MIT-LICENSE",
            "../../packages/native/npm/*/TAILWINDCSS-MIT-LICENSE",
            "../../packages/native/npm/*/WEBREF-MIT-LICENSE",
            "../../packages/vscode-extension/WEBREF-MIT-LICENSE",
          ],
        },
      },
      "check:generated:license-copies": {
        command: "node license-copies.ts --check",
        cache: {
          input: [
            { auto: true },
            "../../LICENSE",
            "../../TAILWINDCSS-MIT-LICENSE",
            "../../WEBREF-MIT-LICENSE",
            "../../packages/vscode-extension/LICENSE",
            "../../packages/*/TAILWINDCSS-MIT-LICENSE",
            "../../packages/native/npm/*/TAILWINDCSS-MIT-LICENSE",
            "../../packages/native/npm/*/WEBREF-MIT-LICENSE",
            "../../packages/vscode-extension/WEBREF-MIT-LICENSE",
          ],
        },
      },
      typecheck: {
        command: "tsc --noEmit",
      },
    },
  },
});
