import { defineConfig } from "vite-plus";

export default defineConfig({
  lint: {
    ignorePatterns: [
      "fixtures/projects/*/expected/**",
      "apps/fixtures-viewer/public/fixtures.json",
      "experiments/sfc-projection-research/fixtures/**",
      "experiments/sfc-projection-research/results/**",
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
    // Raw converter output and offset-sensitive inputs are research evidence.
    ignorePatterns: [
      "experiments/sfc-projection-research/fixtures/**",
      "experiments/sfc-projection-research/results/**",
      "packages/native/binding.d.ts",
    ],
    sortImports: true,
    sortPackageJson: true,
  },
  run: {
    tasks: {
      "perf:quick": {
        command: "node experiments/performance-quick/run.mjs",
        dependsOn: ["@stillwind/native#build"],
        cache: false,
      },
      "perf:host": {
        command: "node experiments/performance-quick/host.mts",
        dependsOn: ["@stillwind/host#build"],
        cache: false,
      },
      "perf:profile": {
        command: "node experiments/performance-quick/profile.mjs",
        cache: false,
      },
      "perf:summarize": {
        command: "node experiments/performance-quick/summarize.mjs",
        cache: false,
      },
      "sourcemap:build": {
        command:
          "cargo build --manifest-path experiments/source-map-research/native/Cargo.toml --release --locked",
        cache: false,
      },
      "sourcemap:native-test": {
        command:
          "cargo test --manifest-path experiments/source-map-research/native/Cargo.toml --release --locked --bin stillwind-sourcemap-research",
        cache: false,
      },
      "sourcemap:research": {
        command: "node experiments/source-map-research/run.mjs",
        dependsOn: ["sourcemap:build"],
        cache: false,
      },
      "sourcemap:acceptance": {
        command: "node experiments/source-map-research/acceptance.mjs",
        dependsOn: ["@stillwind/native#build"],
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
      "ts7-shadow:poc": {
        command: "node experiments/ts7-shadow-checker/src/index.mjs",
        cache: false,
      },
    },
  },
});
