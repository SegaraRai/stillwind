import { testCache } from "@stillwind-tools/workspace/task-cache";
import { defineConfig } from "vite-plus";

export default defineConfig({
  root: import.meta.dirname,
  pack: {
    entry: [
      "src/css.ts",
      "src/module-css.ts",
      "src/macro/index.ts",
      "src/frameworks/react.ts",
      "src/frameworks/preact.ts",
      "src/frameworks/solid.ts",
      "src/frameworks/vue.ts",
      "src/frameworks/svelte.ts",
      "src/frameworks/astro.ts",
      "src/runtime/react.ts",
      "src/runtime/preact.ts",
      "src/runtime/solid.ts",
      "src/runtime/vue.ts",
      "src/runtime/svelte.ts",
      "src/runtime/astro.ts",
      "src/runtime/dev.ts",
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
