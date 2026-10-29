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
      "src/frameworks/alpine.ts",
      "src/frameworks/astro.ts",
      "src/frameworks/lit.ts",
      "src/frameworks/preact.ts",
      "src/frameworks/qwik.ts",
      "src/frameworks/react.ts",
      "src/frameworks/solid.ts",
      "src/frameworks/svelte.ts",
      "src/frameworks/vanilla.ts",
      "src/frameworks/vue.ts",
      "src/runtime/class-name-object.ts",
      "src/runtime/class-name-object-development.ts",
      "src/runtime/class-object.ts",
      "src/runtime/class-object-development.ts",
      "src/runtime/class-string.ts",
      "src/runtime/class-string-development.ts",
    ],
    format: ["esm"],
    fixedExtension: false,
    dts: true,
    outputOptions: {
      // The semantic checker discovers asset brands through exported type names.
      minifyInternalExports: false,
    },
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
        dependsOn: [
          "@stillwind/config#build",
          "@stillwind/native#build",
          "@stillwind/projection#build",
        ],
      },
      typecheck: {
        command: "tsc --noEmit",
        dependsOn: ["build"],
      },
      "prepare-publish": {
        command: "",
        dependsOn: ["build", "@stillwind-internal/apps-website#generate:package-docs"],
      },
      test: {
        command: "vp test --run",
        cache: testCache,
        dependsOn: ["build"],
      },
    },
  },
});
