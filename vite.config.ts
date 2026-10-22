import { rustCache, testCache } from "@stillwind-internal/tools-workspace/task-cache";
import { defineConfig } from "vite-plus";

export default defineConfig(({ mode }) => ({
  fmt: {
    ignorePatterns: ["packages/native/binding.d.ts"],
    sortImports: true,
    sortPackageJson: true,
    astro: true,
    svelte: true,
  },
  lint: {
    categories: {
      correctness: "error",
    },
    ignorePatterns: [
      "apps/fixtures-viewer/public/fixtures.json",
      "fixtures/projects/*/expected/**",
      "packages/native/binding.d.ts",
    ],
    options: {
      denyWarnings: true,
      reportUnusedDisableDirectives: "error",
      typeAware: true,
    },
    rules: {
      curly: "error",
      eqeqeq: [
        "error",
        "always",
        {
          null: "ignore",
        },
      ],
      "no-unused-vars": [
        "error",
        {
          args: "all",
          argsIgnorePattern: "^_",
          caughtErrors: "all",
          ignoreRestSiblings: true,
          vars: "all",
        },
      ],
      "no-var": "error",
      "prefer-const": "error",
    },
  },
  test: {
    projects:
      mode === "wasm"
        ? [
            "e2e/harness/vite.browser.config.ts",
            "packages/config/vite.config.ts",
            "packages/native/vite.config.ts",
          ]
        : mode === "browser"
          ? [
              "e2e/css/vite.browser.config.ts",
              "packages/vscode-extension/vite.config.ts",
              "tools/tailwind-compat/vite.browser.config.ts",
            ]
          : [
              "apps/logo-editor/vite.config.ts",
              "apps/play/vite.config.ts",
              "apps/website/vite.config.ts",
              "benchmarks/vite.config.ts",
              "e2e/harness/vite.config.ts",
              "packages/analysis/vite.config.ts",
              "packages/cli/vite.config.ts",
              "packages/config/vite.config.ts",
              "packages/core/vite.config.ts",
              "packages/host/vite.config.ts",
              "packages/integration/vite.config.ts",
              "packages/language-server/vite.config.ts",
              "packages/native/vite.config.ts",
              "packages/prettier-plugin/vite.config.ts",
              "packages/projection/vite.config.ts",
              "packages/vscode-extension/vite.config.ts",
              "plugins/iconify/vite.config.ts",
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
          "@stillwind-internal/tools-workspace#generate:lint-config",
          "generate:fixtures:projects",
          "generate:fixtures:tailwind",
          "generate:tailwind-order",
          "generate:webref",
          "@stillwind-internal/tools-tailwind#generate:fixtures:tailwind-order",
        ],
      },
      "generate:fixtures:projects":
        "vp run @stillwind-internal/tools-projects#generate:fixtures:projects",
      "generate:fixtures:tailwind":
        "vp run @stillwind-internal/tools-tailwind#generate:fixtures:tailwind",
      "generate:tailwind-order":
        "vp run @stillwind-internal/tools-tailwind#generate:tailwind-order",
      "generate:webref": "vp run @stillwind-internal/tools-webref#generate:webref",
      // dev
      "dev:play": "vp run @stillwind-internal/apps-play#dev",
      "dev:web": "vp run @stillwind-internal/apps-website#dev",
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
          "@stillwind-internal/e2e-nuxt4-vue3#generate:types",
          "@stillwind-internal/e2e-sveltekit2-svelte5#generate:types",
          "@stillwind-internal/e2e-sveltekit3-svelte5#generate:types",
          "@stillwind/cli#build",
          "@stillwind/host#build",
          "@stillwind/integration#build",
          "@stillwind/native#build",
          "@stillwind/plugin-iconify#build",
          "@stillwind/projection#build",
          "prettier-plugin-stillwind#build",
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
          "@stillwind-internal/tools-workspace#check:generated:lint-config",
          "@stillwind-internal/tools-tailwind#check:generated:fixtures:tailwind-order",
          "@stillwind-internal/tools-tailwind#check:generated:fixtures:tailwind",
          "@stillwind-internal/tools-tailwind#check:generated:tailwind-order",
          "@stillwind-internal/tools-webref#check:generated:webref",
        ],
      },
      // typecheck
      "typecheck:root": {
        command: "tsc --noEmit",
      },
      typecheck: {
        command: "",
        dependsOn: [
          "typecheck:root",
          "@stillwind-internal/apps-fixtures-viewer#typecheck",
          "@stillwind-internal/apps-logo-editor#typecheck",
          "@stillwind-internal/apps-play#typecheck",
          "@stillwind-internal/apps-website#typecheck",
          "@stillwind-internal/benchmarks#typecheck",
          "@stillwind-internal/e2e-harness#typecheck:fixtures",
          "@stillwind-internal/e2e-harness#typecheck",
          "@stillwind-internal/e2e-iconify-plugin#typecheck",
          "@stillwind-internal/e2e-play#typecheck",
          "@stillwind-internal/e2e-tailwind-plugins#typecheck",
          "@stillwind-internal/tools-fixture-css#typecheck",
          "@stillwind-internal/tools-projects#typecheck",
          "@stillwind-internal/tools-tailwind-compat#typecheck",
          "@stillwind-internal/tools-tailwind#typecheck",
          "@stillwind-internal/tools-webref#typecheck",
          "@stillwind-internal/tools-workspace#typecheck",
          "@stillwind/analysis#typecheck",
          "@stillwind/cli#typecheck",
          "@stillwind/config#typecheck",
          "@stillwind/host#typecheck",
          "@stillwind/integration#typecheck",
          "@stillwind/language-server#typecheck",
          "@stillwind/native#typecheck",
          "@stillwind/plugin-iconify#typecheck",
          "@stillwind/projection#typecheck",
          "prettier-plugin-stillwind#typecheck",
          "@stillwind/vscode-extension#typecheck",
          "stillwind#typecheck",
        ],
      },
      // verify
      verify: {
        command: "",
        dependsOn: ["check", "test"],
      },
      // bench
      "bench:architecture": "vp run @stillwind-internal/benchmarks#bench:architecture",
      "bench:compiler": "vp run @stillwind-internal/benchmarks#bench:compiler",
      "bench:compiler:browser": "vp run @stillwind-internal/benchmarks#bench:compiler:browser",
      "bench:compiler:wasm": "vp run @stillwind-internal/benchmarks#bench:compiler:wasm",
      "bench:host": "vp run @stillwind-internal/benchmarks#bench:host",
      "bench:sorting": "vp run @stillwind-internal/benchmarks#bench:sorting",
      "bench:sorting:wasm": "vp run @stillwind-internal/benchmarks#bench:sorting:wasm",
      "bench:sorting:profile": "vp run @stillwind-internal/benchmarks#bench:sorting:profile",
      "bench:source-map": "vp run @stillwind-internal/benchmarks#bench:source-map",
      // test
      test: {
        command: "",
        dependsOn: ["test:js", "test:rust", "test:fixtures:projects"],
      },
      "test:js": {
        command: "vp test --run",
        cache: testCache,
        dependsOn: [
          "@stillwind-internal/apps-website#build",
          "@stillwind/cli#build",
          "@stillwind/host#build",
          "@stillwind/integration#build",
          "@stillwind/native#build",
          "@stillwind/native#collect:local",
          "@stillwind/native#typecheck",
          "@stillwind/projection#build",
          "prettier-plugin-stillwind#build",
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
      "test:fixtures:projects": "vp run @stillwind-internal/tools-projects#test:fixtures:projects",
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
        dependsOn: ["@stillwind/native#collect:local", "@stillwind/vscode-extension#build"],
      },
      "test:wasm": {
        command: "vp test --run --mode wasm",
        cache: false,
        dependsOn: ["@stillwind/language-server#build", "@stillwind/native#collect:local"],
      },
      "test:performance": "vp run @stillwind-internal/benchmarks#test:performance",
      "test:performance:full": "vp run @stillwind-internal/benchmarks#test:performance:full",
      "test:e2e": {
        command: "",
        dependsOn: [
          "@stillwind-internal/e2e-harness#test:e2e",
          "@stillwind-internal/e2e-play#test:e2e",
        ],
      },
      "test:e2e:play": "vp run @stillwind-internal/e2e-play#test:e2e",
      "test:e2e:primary": "vp run @stillwind-internal/e2e-harness#test:e2e:primary",
      "test:e2e:compatibility": "vp run @stillwind-internal/e2e-harness#test:e2e:compatibility",
    },
  },
}));
