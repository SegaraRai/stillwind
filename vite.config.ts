import { rustCache, testCache } from "@stillwind-tools/workspace/task-cache";
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
        ? ["packages/native/vite.config.ts", "e2e/harness/vite.browser.config.ts"]
        : mode === "browser"
          ? ["tools/tailwind-compat/vite.browser.config.ts", "e2e/css/vite.browser.config.ts"]
          : [
              "packages/core/vite.config.ts",
              "packages/native/vite.config.ts",
              "packages/host/vite.config.ts",
              "packages/integration/vite.config.ts",
              "tools/tailwind/vite.config.ts",
              "tools/tailwind-compat/vite.config.ts",
              "benchmarks/vite.config.ts",
              "e2e/harness/vite.config.ts",
              "apps/play/vite.config.ts",
              "apps/logo-editor/vite.config.ts",
            ],
  },
  run: {
    tasks: {
      // generate
      generate: {
        command: "",
        dependsOn: [
          "generate:webref",
          "generate:tailwind-order",
          "generate:fixtures:projects",
          "generate:fixtures:tailwind",
          "@stillwind-tools/tailwind#generate:fixtures:tailwind-order",
        ],
      },
      "generate:fixtures:projects": {
        command: "",
        dependsOn: ["@stillwind-tools/projects#generate:fixtures:projects"],
      },
      "generate:fixtures:tailwind": {
        command: "",
        dependsOn: ["@stillwind-tools/tailwind#generate:fixtures:tailwind"],
      },
      "generate:tailwind-order": {
        command: "",
        dependsOn: ["@stillwind-tools/tailwind#generate:tailwind-order"],
      },
      "generate:webref": {
        command: "",
        dependsOn: ["@stillwind-tools/webref#generate:webref"],
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
        dependsOn: ["@stillwind/integration#build"],
      },
      "lint:rust": {
        command: "cargo clippy --workspace --all-targets -- -D warnings",
        cache: rustCache,
        dependsOn: [
          "@stillwind-tools/webref#check:generated:webref",
          "@stillwind-tools/tailwind#check:generated:tailwind-order",
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
          "@stillwind-tools/webref#check:generated:webref",
          "@stillwind-tools/tailwind#check:generated:fixtures:tailwind",
          "@stillwind-tools/tailwind#check:generated:tailwind-order",
          "@stillwind-tools/tailwind#check:generated:fixtures:tailwind-order",
        ],
      },
      // typecheck
      typecheck: {
        command: "",
        dependsOn: [
          "stillwind#typecheck",
          "@stillwind/benchmarks#typecheck",
          "@stillwind/e2e-harness#typecheck:fixtures",
          "@stillwind/e2e-harness#typecheck",
          "@stillwind/fixtures-viewer#typecheck",
          "@stillwind/host#typecheck",
          "@stillwind/integration#typecheck",
          "@stillwind/logo-editor#typecheck",
          "@stillwind/native#typecheck",
          "@stillwind/play#typecheck",
          "@stillwind/projection#typecheck",
          "@stillwind/react-example#typecheck",
          "@stillwind-tools/fixture-css#typecheck",
          "@stillwind-tools/projects#typecheck",
          "@stillwind-tools/tailwind-compat#typecheck",
          "@stillwind-tools/tailwind#typecheck",
          "@stillwind-tools/webref#typecheck",
          "@stillwind-tools/workspace#typecheck",
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
        dependsOn: ["@stillwind/benchmarks#bench:architecture"],
      },
      "bench:compiler": {
        command: "",
        dependsOn: ["@stillwind/benchmarks#bench:compiler"],
      },
      "bench:host": {
        command: "",
        dependsOn: ["@stillwind/benchmarks#bench:host"],
      },
      "bench:research": {
        command: "",
        dependsOn: ["@stillwind/benchmarks#bench:research"],
      },
      "bench:source-map": {
        command: "",
        dependsOn: ["@stillwind/benchmarks#bench:source-map"],
      },
      // test
      test: {
        command: "",
        dependsOn: ["test:js", "test:rust", "test:fixtures:projects"],
      },
      "test:js": {
        command: "vp test --run",
        cache: testCache,
        dependsOn: ["@stillwind/integration#build", "@stillwind/native#typecheck"],
      },
      "test:rust": {
        command: "cargo test --workspace",
        cache: {
          ...rustCache,
          input: [
            ...rustCache.input,
            { pattern: "fixtures/tailwind-v4/utility-order.json", base: "workspace" },
          ],
        },
        dependsOn: [
          "@stillwind-tools/tailwind#generate:tailwind-order",
          "@stillwind-tools/webref#generate:webref",
        ],
      },
      "test:fixtures:projects": {
        command: "",
        dependsOn: ["@stillwind-tools/projects#test:fixtures:projects"],
      },
      "test:tailwind": {
        command: "",
        dependsOn: [
          "@stillwind-tools/tailwind-compat#test:tailwind:parity",
          "@stillwind-tools/tailwind-compat#test:tailwind:property-order",
          "@stillwind-tools/tailwind-compat#test:tailwind:numeric-domains",
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
        dependsOn: ["@stillwind/benchmarks#test:performance"],
      },
      "test:performance:full": {
        command: "",
        dependsOn: ["@stillwind/benchmarks#test:performance:full"],
      },
      "test:e2e": {
        command: "",
        dependsOn: ["@stillwind/e2e-harness#test:e2e"],
      },
      "test:e2e:primary": {
        command: "",
        dependsOn: ["@stillwind/e2e-harness#test:e2e:primary"],
      },
      "test:e2e:compatibility": {
        command: "",
        dependsOn: ["@stillwind/e2e-harness#test:e2e:compatibility"],
      },
    },
  },
}));
