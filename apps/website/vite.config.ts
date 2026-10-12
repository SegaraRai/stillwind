import { testCache } from "@stillwind-internal/tools-workspace/task-cache";
import { defineConfig } from "vite-plus";

export default defineConfig({
  root: import.meta.dirname,
  test: {
    name: "website",
    include: ["src/**/*.test.ts"],
  },
  run: {
    tasks: {
      verify: {
        command: "",
        dependsOn: ["build", "typecheck", "test"],
      },
      dev: {
        command: "astro dev",
        cache: false,
        dependsOn: ["@stillwind/integration#build"],
      },
      build: {
        command: "astro build",
        dependsOn: ["generate:types"],
        cache: {
          input: [
            { auto: true },
            {
              pattern: "packages/core/css/**",
              base: "workspace",
            },
            "!.astro/**",
            "!dist/**",
          ],
          output: ["dist/**"],
        },
      },
      preview: {
        command: "astro preview --port 4322",
        cache: false,
      },
      "generate:types": {
        command: "astro sync",
        dependsOn: ["@stillwind/integration#build"],
        cache: {
          input: [{ auto: true }, "!.astro/**"],
          output: [".astro/**/*.d.ts"],
        },
      },
      typecheck: {
        command: "astro check --noSync",
        dependsOn: ["generate:types"],
      },
      test: {
        command: "vp test --run",
        cache: {
          ...testCache,
          input: [
            ...testCache.input,
            {
              pattern: "packages/core/css/**",
              base: "workspace",
            },
          ],
        },
        dependsOn: ["@stillwind/host#build", "stillwind#build"],
      },
    },
  },
});
