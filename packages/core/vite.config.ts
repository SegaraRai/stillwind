import { testCache } from "@stillwind-internal/tools-workspace/task-cache";
import { defineConfig } from "vite-plus";

export default defineConfig({
  root: import.meta.dirname,
  pack: {
    entry: [
      "src/config.ts",
      "src/css.ts",
      "src/module-css.ts",
      "src/macro/index.ts",
      "src/frameworks/astro.ts",
      "src/frameworks/preact.ts",
      "src/frameworks/react.ts",
      "src/frameworks/solid.ts",
      "src/frameworks/svelte.ts",
      "src/frameworks/vue.ts",
      "src/runtime/astro.ts",
      "src/runtime/dev.ts",
      "src/runtime/preact.ts",
      "src/runtime/react.ts",
      "src/runtime/solid.ts",
      "src/runtime/svelte.ts",
      "src/runtime/vue.ts",
    ],
    format: ["esm"],
    fixedExtension: false,
    dts: true,
    sourcemap: true,
  },
  test: {
    name: "core",
    include: ["src/**/*.test.ts", "tests/**/*.test.ts"],
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
    },
  },
});
