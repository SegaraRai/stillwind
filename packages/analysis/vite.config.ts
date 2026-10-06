import { testCache } from "@stillwind-internal/tools-workspace/task-cache";
import { defineConfig } from "vite-plus";

export default defineConfig({
  root: import.meta.dirname,
  pack: {
    entry: ["src/index.ts", "src/memory.ts", "src/node.ts", "src/worker.ts"],
    format: ["esm"],
    fixedExtension: false,
    dts: true,
    sourcemap: true,
  },
  test: {
    name: "analysis",
    include: ["src/**/*.test.ts", "tests/**/*.test.ts"],
    fileParallelism: false,
    testTimeout: 120_000,
  },
  run: {
    tasks: {
      build: {
        command: "vp pack",
        dependsOn: ["@stillwind/host#build"],
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
    },
  },
});
