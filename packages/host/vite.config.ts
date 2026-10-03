import { testCache } from "@stillwind-tools/workspace/task-cache";
import { defineConfig } from "vite-plus";

export default defineConfig({
  root: import.meta.dirname,
  pack: {
    entry: ["src/index.ts"],
    format: ["esm"],
    fixedExtension: false,
    dts: true,
    sourcemap: true,
  },
  test: {
    name: "host",
    include: ["src/**/*.test.ts", "tests/**/*.test.ts"],
    fileParallelism: false,
    testTimeout: 60_000,
  },
  run: {
    tasks: {
      build: {
        command: "vp pack",
        dependsOn: ["@stillwind/native#build", "@stillwind/projection#build", "stillwind#build"],
      },
      typecheck: {
        command: "tsc --noEmit",
        dependsOn: ["@stillwind/native#build", "@stillwind/projection#build", "stillwind#build"],
      },
      test: {
        command: "vp test --run",
        cache: testCache,
        dependsOn: ["build"],
      },
    },
  },
});
