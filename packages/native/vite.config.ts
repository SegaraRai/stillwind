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
      "sync-platform-packages": {
        command: "napi create-npm-dirs --package-json-path package.json --npm-dir npm",
        cache: false,
      },
      build: {
        command: "",
        dependsOn: ["build:binding"],
      },
      "build:binding": {
        command:
          "napi build --release --platform --esm --no-const-enum --manifest-path ../../crates/napi/Cargo.toml --output-dir dist --dts ../binding.d.ts --package-json-path package.json",
      },
      "build:wasm": {
        command:
          "napi build --release --platform --esm --no-const-enum --target wasm32-wasip1-threads --manifest-path ../../crates/napi/Cargo.toml --output-dir dist --dts ../binding.d.ts --package-json-path package.json",
        dependsOn: ["build:binding"],
      },
      "collect-artifacts": {
        command:
          "napi artifacts --package-json-path package.json --output-dir dist --build-output-dir dist --npm-dir npm",
        cache: false,
      },
      "collect:local": {
        command: "node collect-local-artifacts.ts",
        dependsOn: ["build:wasm"],
        cache: false,
      },
      "test:wasm": {
        command: "vp test --run --mode wasm",
        cache: false,
        dependsOn: ["collect:local"],
      },
      typecheck: {
        command: "tsc --noEmit",
        dependsOn: ["build"],
      },
      test: {
        command: "vp test --run",
        cache: false,
        dependsOn: ["typecheck"],
      },
    },
  },
}));
