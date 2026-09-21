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
      "crates:check": {
        command: "",
        dependsOn: ["@stillwind/tools#check"],
      },
      fmt: {
        command: "vp fmt --check",
      },
      "fmt:write": {
        command: "vp fmt",
      },
      "webref:generate": {
        command: "",
        dependsOn: ["@stillwind/tools#webref:generate"],
      },
      "webref:check": {
        command: "",
        dependsOn: ["@stillwind/tools#webref:check"],
      },
      lint: {
        command: "vp lint",
        dependsOn: ["@stillwind/integration#build"],
      },
      check: {
        command: "vp check",
        dependsOn: ["@stillwind/integration#build"],
      },
      "ts7-shadow:poc": {
        command: "node experiments/ts7-shadow-checker/src/index.mjs",
      },
      "fixtures:refresh": {
        command: "",
        dependsOn: ["@stillwind/tools#review:refresh"],
      },
      "fixtures:check": {
        command: "",
        dependsOn: ["@stillwind/tools#review:check"],
      },
      "tailwind:parity": {
        command: "",
        dependsOn: ["@stillwind/tools#tailwind:parity"],
      },
      "tailwind:parity:all": {
        command: "",
        dependsOn: ["@stillwind/tools#tailwind:parity:all"],
      },
      "tailwind:property-order": {
        command: "",
        dependsOn: ["@stillwind/tools#tailwind:property-order"],
      },
      "tailwind:property-order:test": {
        command: "",
        dependsOn: ["@stillwind/tools#tailwind:property-order:test"],
      },
      "tailwind:parity:analyze": {
        command: "",
        dependsOn: ["@stillwind/tools#tailwind:parity:analyze"],
      },
      "tailwind:numeric-domain-audit": {
        command: "",
        dependsOn: ["@stillwind/tools#tailwind:numeric-domain-audit"],
      },
      "tailwind:registry-audit": {
        command: "",
        dependsOn: ["@stillwind/tools#tailwind:registry-audit"],
      },
    },
  },
});
