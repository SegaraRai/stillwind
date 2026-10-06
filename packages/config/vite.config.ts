import { testCache } from "@stillwind-internal/tools-workspace/task-cache";
import { defineConfig } from "vite-plus";

export default defineConfig({
  root: import.meta.dirname,
  pack: {
    entry: ["src/index.ts", "src/node.ts"],
    format: ["esm"],
    fixedExtension: false,
    dts: true,
    sourcemap: true,
  },
  test: { name: "config", include: ["src/**/*.test.ts"] },
  run: {
    tasks: {
      build: {
        command: "vp pack",
        dependsOn: ["@stillwind/native#build", "@stillwind/projection#build"],
      },
      typecheck: { command: "tsc --noEmit", dependsOn: ["build"] },
      test: { command: "vp test --run", cache: testCache, dependsOn: ["build"] },
    },
  },
});
