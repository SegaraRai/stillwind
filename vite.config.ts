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
      "fixtures/projects/*/expected/**",
      "apps/fixtures-viewer/public/fixtures.json",
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
          ? ["tools/vite.browser.config.ts", "e2e/css/vite.browser.config.ts"]
          : [
              "packages/core/vite.config.ts",
              "packages/native/vite.config.ts",
              "packages/host/vite.config.ts",
              "packages/integration/vite.config.ts",
              "tools/vite.config.ts",
              "benchmarks/vite.config.ts",
              "e2e/harness/vite.config.ts",
              "apps/play/vite.config.ts",
              "apps/logo-editor/vite.config.ts",
            ],
  },
  run: {
    tasks: {
      "verify:performance": {
        command: "",
        dependsOn: ["@stillwind/benchmarks#performance"],
        cache: false,
      },
      "verify:performance:full": {
        command: "",
        dependsOn: ["@stillwind/benchmarks#performance:full"],
        cache: false,
      },
      "bench:compiler": {
        command: "",
        dependsOn: ["@stillwind/benchmarks#compiler"],
        cache: false,
      },
      "bench:host": {
        command: "",
        dependsOn: ["@stillwind/benchmarks#host"],
        cache: false,
      },
      "bench:source-map": {
        command: "",
        dependsOn: ["@stillwind/benchmarks#source-map"],
        cache: false,
      },
      "verify:e2e": {
        command: "",
        dependsOn: ["@stillwind/e2e-harness#e2e"],
        cache: false,
      },
      "verify:e2e:primary": {
        command: "",
        dependsOn: ["@stillwind/e2e-harness#e2e:primary"],
        cache: false,
      },
      "verify:e2e:compatibility": {
        command: "",
        dependsOn: ["@stillwind/e2e-harness#e2e:compatibility"],
        cache: false,
      },
      fmt: {
        command: "vp fmt --check",
        cache: false,
      },
      "fmt:write": {
        command: "vp fmt",
        cache: false,
      },
      lint: {
        command: "vp lint",
        dependsOn: ["@stillwind/integration#build"],
        cache: false,
      },
      "rust:fmt": {
        command: "cargo fmt --all --check",
        cache: false,
      },
      "rust:lint": {
        command: "cargo clippy --workspace --all-targets -- -D warnings",
        cache: false,
      },
      "rust:test": {
        command: "cargo test --workspace",
        cache: false,
      },
      check: {
        command: "",
        dependsOn: [
          "fmt",
          "lint",
          "rust:fmt",
          "rust:lint",
          "stillwind#typecheck",
          "@stillwind/native#typecheck",
          "@stillwind/host#typecheck",
          "@stillwind/projection#typecheck",
          "@stillwind/integration#typecheck",
          "@stillwind/tools#typecheck",
          "@stillwind/benchmarks#typecheck",
          "@stillwind/e2e-harness#check",
          "@stillwind/play#typecheck",
          "@stillwind/logo-editor#check",
          "@stillwind/fixtures-viewer#check",
          "@stillwind/react-example#check",
        ],
      },
      "test:js": {
        command: "vp test --run",
        dependsOn: ["@stillwind/integration#build", "@stillwind/native#typecheck"],
        cache: false,
      },
      test: {
        command: "",
        dependsOn: ["rust:test", "test:js"],
      },
      "fixtures:refresh": {
        command: "",
        dependsOn: ["@stillwind/tools#projects:refresh"],
        cache: false,
      },
      "fixtures:check": {
        command: "",
        dependsOn: [
          "@stillwind/tools#projects:check",
          "@stillwind/tools#tailwind:fixtures:check",
          "@stillwind/tools#webref:check",
          "@stillwind/tools#tailwind:order:check",
        ],
        cache: false,
      },
      verify: {
        command: "",
        dependsOn: ["check", "test", "fixtures:check"],
      },
      "verify:compat": {
        command: "",
        dependsOn: [
          "@stillwind/tools#tailwind:parity",
          "@stillwind/tools#tailwind:property-order",
          "@stillwind/tools#tailwind:numeric-domains",
        ],
        cache: false,
      },
      "verify:browser": {
        command: "vp test --run --mode browser",
        dependsOn: ["@stillwind/native#collect:local"],
        cache: false,
      },
      "verify:wasm": {
        command: "vp test --run --mode wasm",
        dependsOn: ["@stillwind/native#collect:local"],
        cache: false,
      },
      "webref:generate": {
        command: "",
        dependsOn: ["@stillwind/tools#webref:generate"],
        cache: false,
      },
    },
  },
}));
