import { testCache } from "@stillwind-internal/tools-workspace/task-cache";
import { defineConfig } from "vite-plus";

export default defineConfig(({ mode }) => ({
  root: import.meta.dirname,
  pack: {
    entry: ["src/index.ts", "src/browser.ts", "src/node.ts"],
    format: ["esm"],
    fixedExtension: false,
    dts: true,
    sourcemap: true,
  },
  test: {
    name: mode === "wasm" ? "config-wasm" : "config",
    pool: "forks",
    include:
      mode === "wasm"
        ? ["tests/lint.wasm.test.ts"]
        : ["src/**/*.test.ts", "tests/lint.native.test.ts"],
  },
  run: {
    tasks: {
      build: {
        command: "vp pack",
        dependsOn: ["@stillwind/native#build", "@stillwind/projection#build"],
      },
      typecheck: {
        command: "tsc --noEmit",
        dependsOn: ["build"],
      },
      test: {
        command: "vp test --run",
        cache: testCache,
        dependsOn: ["build"],
      },
      "test:wasm": {
        command: "vp test --run --mode wasm",
        cache: false,
        dependsOn: ["build", "@stillwind/native#collect:local"],
      },
    },
  },
}));
