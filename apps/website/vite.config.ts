import { astroCache, testCache } from "@stillwind-internal/tools-workspace/task-cache";
import { defineConfig } from "vite-plus";

export default defineConfig({
  root: import.meta.dirname,
  test: {
    name: "website",
    include: ["src/**/*.test.ts", "tests/**/*.test.ts"],
    css: true,
    fileParallelism: false,
    testTimeout: 60_000,
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
        dependsOn: ["@stillwind/integration#build", "@stillwind/plugin-iconify#build"],
      },
      build: {
        command: "astro build",
        dependsOn: ["typecheck", "@stillwind/native#collect:local"],
        cache: {
          ...astroCache,
          input: [
            ...astroCache.input,
            {
              pattern: "packages/core/css/**",
              base: "workspace",
            },
            "!.astro",
            "!.astro/**",
            "!.nimbus",
            "!.nimbus/**",
            "!dist",
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
        dependsOn: ["@stillwind/integration#build", "@stillwind/plugin-iconify#build"],
        cache: {
          ...astroCache,
          input: [
            ...astroCache.input,
            "!.astro",
            "!.astro/**",
            "!.nimbus",
            "!.nimbus/**",
            "!node_modules/.astro",
            "!node_modules/.astro/**",
          ],
          output: [".astro/**", ".nimbus/**", "node_modules/.astro/**"],
        },
      },
      "generate:package-docs": {
        command: "node package-docs.ts",
        dependsOn: ["build"],
        cache: {
          input: [{ auto: true }, "dist/docs/**/index.md"],
          output: ["../../packages/core/docs/**"],
        },
      },
      typecheck: {
        command: "astro check --noSync",
        cache: astroCache,
        dependsOn: ["generate:types"],
      },
      test: {
        command: "vp test --run",
        cache: testCache,
        dependsOn: ["@stillwind/host#build", "stillwind#build"],
      },
    },
  },
});
