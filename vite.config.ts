import { defineConfig } from "vite-plus";

export default defineConfig({
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
  fmt: {
    ignorePatterns: ["packages/native/binding.d.ts"],
    sortImports: true,
    sortPackageJson: true,
    svelte: true,
  },
  run: {
    tasks: {
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
      test: {
        command: "",
        dependsOn: [
          "rust:test",
          "stillwind#test",
          "@stillwind/native#test",
          "@stillwind/host#test",
          "@stillwind/integration#test",
          "@stillwind/tools#test",
          "@stillwind/benchmarks#test",
          "@stillwind/e2e-harness#test",
          "@stillwind/play#test",
          "@stillwind/logo-editor#test",
        ],
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
        command: "",
        dependsOn: ["@stillwind/tools#tailwind:browser"],
        cache: false,
      },
      "verify:wasm": {
        command: "",
        dependsOn: ["@stillwind/native#test:wasm"],
        cache: false,
      },
      "webref:generate": {
        command: "",
        dependsOn: ["@stillwind/tools#webref:generate"],
        cache: false,
      },
    },
  },
});
