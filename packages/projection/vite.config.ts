import { testCache } from "@stillwind-tools/workspace/task-cache";
import { defineConfig } from "vite-plus";

export default defineConfig({
  root: import.meta.dirname,
  test: {
    name: "projection",
    include: ["src/**/*.test.ts"],
    testTimeout: 30_000,
  },
  pack: {
    entry: ["src/index.ts"],
    format: ["esm"],
    fixedExtension: false,
    dts: true,
    sourcemap: true,
  },
  run: {
    tasks: {
      build: {
        command: "vp pack",
      },
      typecheck: {
        command: "tsc --noEmit",
      },
      test: {
        command: "vp test --run",
        cache: testCache,
      },
    },
  },
});
