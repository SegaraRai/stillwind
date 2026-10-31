import { rustCache, testCache } from "@stillwind-internal/tools-workspace/task-cache";
import { defineConfig } from "vite-plus";

export default defineConfig(({ mode }) => ({
  root: import.meta.dirname,
  test: {
    name: mode === "wasm" ? "native-wasm" : "native",
    environment: "node",
    pool: "forks",
    include:
      mode === "wasm"
        ? ["tests/wasm.test.ts", "tests/package.test.ts", "tests/browser-bundle.test.ts"]
        : ["tests/loader.test.ts", "tests/glibc.test.ts"],
  },
  run: {
    tasks: {
      "generate:rust-licenses": {
        command: "node rust-licenses.ts",
        cache: {
          input: [
            { auto: true },
            { pattern: "Cargo.lock", base: "workspace" },
            { pattern: "crates/*/Cargo.toml", base: "workspace" },
          ],
          output: ["RUST-THIRD-PARTY-LICENSES"],
        },
      },
      "prepare-publish": {
        command: "node prepare-publish.ts",
        dependsOn: ["check:generated:rust-licenses"],
        cache: {
          input: [{ auto: true }, "RUST-THIRD-PARTY-LICENSES"],
          output: ["npm/*/RUST-THIRD-PARTY-LICENSES"],
        },
      },
      "check:generated:rust-licenses": {
        command: "node rust-licenses.ts --check",
        cache: {
          input: [
            { auto: true },
            { pattern: "Cargo.lock", base: "workspace" },
            { pattern: "crates/*/Cargo.toml", base: "workspace" },
            "RUST-THIRD-PARTY-LICENSES",
          ],
          output: [],
        },
      },
      "generate:platform-packages": {
        command:
          "napi create-npm-dirs --package-json-path package.json --npm-dir npm && node generate-platform-packages.ts && vp fmt npm/**",
        cache: false,
      },
      build: {
        command: "node install-binding.ts",
        cache: false,
        dependsOn: ["build:binding"],
      },
      "build:binding": {
        command: "node build-binding.ts",
        cache: {
          ...rustCache,
          env: [...rustCache.env, "NATIVE_USE_NAPI_CROSS"],
          input: [...rustCache.input, "!dist", "!dist/**", "!.build", "!.build/**"],
          output: [".build/native/index.js", ".build/native/*.node", ".build/native/binding.d.ts"],
        },
        dependsOn: [
          "@stillwind-internal/tools-tailwind#generate:tailwind-banner",
          "@stillwind-internal/tools-tailwind#generate:tailwind-order",
          "@stillwind-internal/tools-webref#generate:webref",
        ],
      },
      "build:wasm": {
        command:
          "napi build --release --platform --esm --no-const-enum --target wasm32-wasip1-threads --manifest-path ../../crates/napi/Cargo.toml --output-dir dist/wasm --dts binding.d.ts --package-json-path package.json",
        cache: {
          ...rustCache,
          input: [...rustCache.input, "!dist", "!dist/**", "!.build", "!.build/**"],
          output: ["dist/wasm/**", "!dist/wasm/**/*.tmp"],
        },
        dependsOn: [
          "@stillwind-internal/tools-tailwind#generate:tailwind-banner",
          "@stillwind-internal/tools-tailwind#generate:tailwind-order",
          "@stillwind-internal/tools-webref#generate:webref",
        ],
      },
      "collect:release": {
        command:
          "napi artifacts --package-json-path package.json --output-dir dist --build-output-dir dist/wasm --npm-dir npm",
        cache: false,
      },
      "collect:local": {
        command: "node collect-local-artifacts.ts",
        cache: false,
        dependsOn: ["build", "build:wasm"],
      },
      typecheck: {
        command: "tsc --noEmit",
        dependsOn: ["build"],
      },
      test: {
        command: "vp test --run",
        cache: testCache,
        dependsOn: ["typecheck"],
      },
      "test:wasm": {
        command: "vp test --run --mode wasm",
        cache: false,
        dependsOn: ["collect:local"],
      },
    },
  },
}));
