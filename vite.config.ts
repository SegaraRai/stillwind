import { rustCache, testCache } from "@stillwind-internal/tools-workspace/task-cache";
import { defineConfig } from "vite-plus";

export default defineConfig(({ mode }) => ({
  fmt: {
    ignorePatterns: ["packages/native/binding.d.ts"],
    sortImports: true,
    sortPackageJson: true,
    svelte: true,
  },
  lint: {
    ignorePatterns: [
      "apps/fixtures-viewer/public/fixtures.json",
      "fixtures/projects/*/expected/**",
      "packages/native/binding.d.ts",
    ],
    options: {
      typeAware: true,
    },
    rules: {
      curly: "warn",
    },
  },
  test: {
    projects:
      mode === "wasm"
        ? ["e2e/harness/vite.browser.config.ts", "packages/native/vite.config.ts"]
        : mode === "browser"
          ? ["e2e/css/vite.browser.config.ts", "tools/tailwind-compat/vite.browser.config.ts"]
          : [
              "apps/logo-editor/vite.config.ts",
              "apps/play/vite.config.ts",
              "benchmarks/vite.config.ts",
              "e2e/harness/vite.config.ts",
              "packages/core/vite.config.ts",
              "packages/host/vite.config.ts",
              "packages/integration/vite.config.ts",
              "packages/native/vite.config.ts",
              "packages/projection/vite.config.ts",
              "tools/tailwind-compat/vite.config.ts",
              "tools/tailwind/vite.config.ts",
            ],
  },
  run: {
    tasks: {
      // generate
      generate: {
        command: "",
        dependsOn: [
          "generate:fixtures:projects",
          "generate:fixtures:tailwind",
          "generate:tailwind-order",
          "generate:webref",
          "@stillwind-internal/tools-tailwind#generate:fixtures:tailwind-order",
        ],
      },
      "generate:fixtures:projects": {
        command: "",
        dependsOn: ["@stillwind-internal/tools-projects#generate:fixtures:projects"],
      },
      "generate:fixtures:tailwind": {
        command: "",
        dependsOn: ["@stillwind-internal/tools-tailwind#generate:fixtures:tailwind"],
      },
      "generate:tailwind-order": {
        command: "",
        dependsOn: ["@stillwind-internal/tools-tailwind#generate:tailwind-order"],
      },
      "generate:webref": {
        command: "",
        dependsOn: ["@stillwind-internal/tools-webref#generate:webref"],
      },
      // fmt
      fmt: {
        command: "",
        dependsOn: ["fmt:js", "fmt:rust"],
      },
      "fmt:check": {
        command: "",
        dependsOn: ["fmt:js:check", "fmt:rust:check"],
      },
      "fmt:js": {
        command: "vp fmt",
        cache: false,
      },
      "fmt:js:check": {
        command: "vp fmt --check",
      },
      "fmt:rust": {
        command: "cargo fmt --all",
        cache: false,
      },
      "fmt:rust:check": {
        command: "cargo fmt --all --check",
      },
      // lint
      lint: {
        command: "",
        dependsOn: ["lint:js", "lint:rust"],
      },
      "lint:js": {
        command: "vp lint",
        dependsOn: [
          "@stillwind/host#build",
          "@stillwind/integration#build",
          "@stillwind/native#build",
          "@stillwind/projection#build",
          "stillwind#build",
        ],
      },
      "lint:rust": {
        command: "cargo clippy --workspace --all-targets -- -D warnings",
        cache: rustCache,
        dependsOn: [
          "@stillwind-internal/tools-tailwind#check:generated:tailwind-order",
          "@stillwind-internal/tools-webref#check:generated:webref",
        ],
      },
      // check
      check: {
        command: "",
        dependsOn: ["fmt:check", "lint", "typecheck", "check:generated"],
      },
      "check:generated": {
        command: "",
        dependsOn: [
          "@stillwind-internal/tools-tailwind#check:generated:fixtures:tailwind-order",
          "@stillwind-internal/tools-tailwind#check:generated:fixtures:tailwind",
          "@stillwind-internal/tools-tailwind#check:generated:tailwind-order",
          "@stillwind-internal/tools-webref#check:generated:webref",
        ],
      },
      // typecheck
      typecheck: {
        command: "",
        dependsOn: [
          "@stillwind-internal/apps-fixtures-viewer#typecheck",
          "@stillwind-internal/apps-logo-editor#typecheck",
          "@stillwind-internal/apps-play#typecheck",
          "@stillwind-internal/benchmarks#typecheck",
          "@stillwind-internal/e2e-harness#typecheck:fixtures",
          "@stillwind-internal/e2e-harness#typecheck",
          "@stillwind-internal/tools-fixture-css#typecheck",
          "@stillwind-internal/tools-projects#typecheck",
          "@stillwind-internal/tools-tailwind-compat#typecheck",
          "@stillwind-internal/tools-tailwind#typecheck",
          "@stillwind-internal/tools-webref#typecheck",
          "@stillwind-internal/tools-workspace#typecheck",
          "@stillwind/host#typecheck",
          "@stillwind/integration#typecheck",
          "@stillwind/native#typecheck",
          "@stillwind/projection#typecheck",
          "stillwind#typecheck",
        ],
      },
      // verify
      verify: {
        command: "",
        dependsOn: ["check", "test"],
      },
      // bench
      "bench:architecture": {
        command: "",
        dependsOn: ["@stillwind-internal/benchmarks#bench:architecture"],
      },
      "bench:compiler": {
        command: "",
        dependsOn: ["@stillwind-internal/benchmarks#bench:compiler"],
      },
      "bench:host": {
        command: "",
        dependsOn: ["@stillwind-internal/benchmarks#bench:host"],
      },
      "bench:research": {
        command: "",
        dependsOn: ["@stillwind-internal/benchmarks#bench:research"],
      },
      "bench:source-map": {
        command: "",
        dependsOn: ["@stillwind-internal/benchmarks#bench:source-map"],
      },
      // test
      test: {
        command: "",
        dependsOn: ["test:js", "test:rust", "test:fixtures:projects"],
      },
      "test:js": {
        command: "vp test --run",
        cache: testCache,
        dependsOn: [
          "@stillwind/host#build",
          "@stillwind/integration#build",
          "@stillwind/native#build",
          "@stillwind/native#typecheck",
          "@stillwind/projection#build",
          "stillwind#build",
        ],
      },
      "test:rust": {
        command: "cargo test --workspace",
        cache: {
          ...rustCache,
          input: [
            ...rustCache.input,
            {
              pattern: "fixtures/tailwind-v4/utility-order.json",
              base: "workspace",
            },
          ],
        },
        dependsOn: [
          "@stillwind-internal/tools-tailwind#generate:tailwind-order",
          "@stillwind-internal/tools-webref#generate:webref",
        ],
      },
      "test:fixtures:projects": {
        command: "",
        dependsOn: ["@stillwind-internal/tools-projects#test:fixtures:projects"],
      },
      "test:tailwind": {
        command: "",
        dependsOn: [
          "@stillwind-internal/tools-tailwind-compat#test:tailwind:numeric-domains",
          "@stillwind-internal/tools-tailwind-compat#test:tailwind:parity",
          "@stillwind-internal/tools-tailwind-compat#test:tailwind:property-order",
        ],
      },
      "test:browser": {
        command: "vp test --run --mode browser",
        cache: false,
        dependsOn: ["@stillwind/native#collect:local"],
      },
      "test:wasm": {
        command: "vp test --run --mode wasm",
        cache: false,
        dependsOn: ["@stillwind/native#collect:local"],
      },
      "test:performance": {
        command: "",
        dependsOn: ["@stillwind-internal/benchmarks#test:performance"],
      },
      "test:performance:full": {
        command: "",
        dependsOn: ["@stillwind-internal/benchmarks#test:performance:full"],
      },
      "test:e2e": {
        command: "",
        dependsOn: ["@stillwind-internal/e2e-harness#test:e2e"],
      },
      "test:e2e:primary": {
        command: "",
        dependsOn: ["@stillwind-internal/e2e-harness#test:e2e:primary"],
      },
      "test:e2e:compatibility": {
        command: "",
        dependsOn: ["@stillwind-internal/e2e-harness#test:e2e:compatibility"],
      },
    },
  },
}));
