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
        : ["tests/loader.test.ts"],
  },
  run: {
    tasks: {
      "generate:platform-packages": {
        command:
          "napi create-npm-dirs --package-json-path package.json --npm-dir npm && node generate-platform-readmes.ts && vp fmt npm/**",
        cache: false,
      },
      build: {
        command: "node install-binding.ts",
        cache: false,
        dependsOn: ["build:binding"],
      },
      "build:binding": {
        command:
          "napi build --release --platform --esm --no-const-enum --manifest-path ../../crates/napi/Cargo.toml --output-dir .build/native --dts binding.d.ts --package-json-path package.json",
        cache: {
          ...rustCache,
          input: [...rustCache.input, "!dist/**", "!.build/**"],
          output: [".build/native/index.js", ".build/native/*.node", ".build/native/binding.d.ts"],
        },
        dependsOn: [
          "@stillwind-internal/tools-tailwind#generate:tailwind-order",
          "@stillwind-internal/tools-webref#generate:webref",
        ],
      },
      "build:wasm": {
        command:
          "napi build --release --platform --esm --no-const-enum --target wasm32-wasip1-threads --manifest-path ../../crates/napi/Cargo.toml --output-dir dist/wasm --dts binding.d.ts --package-json-path package.json",
        cache: {
          ...rustCache,
          input: [...rustCache.input, "!dist/**", "!.build/**"],
          output: ["dist/wasm/**", "!dist/wasm/**/*.tmp"],
        },
        dependsOn: [
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
